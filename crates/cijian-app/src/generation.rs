use cijian_domain::{
    analysis::{Analysis, analyze},
    knowledge::Targets,
    settings::{Genre, Tier, ValidatedSettings},
};
use cijian_ports::{
    AppError, ErrorCode, Result,
    provider::{CallStart, CompletionInput, LanguageModel, ProviderSnapshot},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
const ARTICLE_PROMPT: &str = include_str!("../../../prompts/article.v1.md");
const REPAIR_PROMPT: &str = include_str!("../../../prompts/json-repair.v1.md");
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleDraft {
    pub title: String,
    pub body: String,
    pub names: Vec<String>,
}
impl ArticleDraft {
    pub fn parse(text: &str) -> Result<Self> {
        let draft: Self = serde_json::from_str(text)
            .map_err(|_| AppError::new(ErrorCode::LlmOutputInvalid, "文章结构无效"))?;
        if draft.title.trim().is_empty()
            || draft.body.trim().is_empty()
            || draft.names.iter().any(|n| n.trim().is_empty())
        {
            return Err(AppError::new(
                ErrorCode::LlmOutputInvalid,
                "文章标题、正文或人名字段无效",
            ));
        }
        Ok(draft)
    }
}
#[derive(Debug, Clone)]
pub struct GenerationInput {
    pub profile: String,
    pub operation: String,
    pub now: i64,
    pub settings: ValidatedSettings,
    pub tier: Tier,
    pub genre: Genre,
    pub last_auto_genre: Option<Genre>,
    pub targets: Targets,
    pub known: BTreeSet<String>,
    pub provider: ProviderSnapshot,
}
#[derive(Debug, Clone, Serialize)]
pub struct GeneratedArticle {
    pub draft: ArticleDraft,
    pub analysis: Analysis,
    pub genre: Genre,
    pub underlying_calls: u32,
    pub diagnostics: Vec<String>,
}
fn genre_name(genre: Genre) -> &'static str {
    match genre {
        Genre::Auto | Genre::Story => "story",
        Genre::Dialogue => "dialogue",
        Genre::Monologue => "monologue",
    }
}
fn resolve_genre(genre: Genre, last: Option<Genre>) -> Genre {
    if genre != Genre::Auto {
        return genre;
    }
    match last {
        Some(Genre::Story) => Genre::Dialogue,
        Some(Genre::Dialogue) => Genre::Monologue,
        _ => Genre::Story,
    }
}

/// Bounded orchestration over a fixed request snapshot. Persistence surrounds this function.
pub async fn generate(
    model: &impl LanguageModel,
    input: &GenerationInput,
) -> Result<GeneratedArticle> {
    if input.targets.exposed.is_empty() {
        return Err(AppError::new(
            ErrorCode::NoTargets,
            "没有可用于生成的目标词",
        ));
    }
    let settings = input.settings.get();
    let genre = resolve_genre(input.genre, input.last_auto_genre);
    let exposed: BTreeSet<_> = input.targets.exposed.iter().cloned().collect();
    let control: BTreeSet<_> = input.targets.control.iter().cloned().collect();
    if !exposed.is_disjoint(&control) {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "出现组与对照组不能重叠",
        ));
    }
    let (system, template) = ARTICLE_PROMPT
        .split_once("## user")
        .expect("versioned prompt sections");
    let system = system.trim_start_matches("## system").trim().to_owned();
    let mut avoid = Vec::<String>::new();
    let mut calls = 0;
    for attempt in 0..=settings.generation_max_retries {
        let user = template
            .trim()
            .replace("{genre}", genre_name(genre))
            .replace("{minWords}", &settings.article_min_words.to_string())
            .replace("{maxWords}", &settings.article_max_words.to_string())
            .replace("{level}", &format!("{:?}", settings.level))
            .replace("{targets}", &input.targets.exposed.join(", "))
            .replace("{avoidWords}", &avoid.join(", "));
        let call = |index: u32, purpose: &str| CallStart {
            profile_id: input.profile.clone(),
            call_id: format!("{}-call-{index}", input.operation),
            purpose: purpose.into(),
            model: input.provider.config.model.clone(),
            prompt_version: if purpose == "repair" {
                "json-repair.v1"
            } else {
                "article.v1"
            }
            .into(),
            provider_fingerprint: String::new(),
            now: input.now,
        };
        calls += 1;
        let output = model
            .complete(
                &input.provider,
                CompletionInput {
                    system: system.clone(),
                    user: user.clone(),
                    json: true,
                },
                call(calls, "article"),
            )
            .await?;
        let draft = match ArticleDraft::parse(&output) {
            Ok(draft) => draft,
            Err(_) => {
                calls += 1;
                let repair = REPAIR_PROMPT
                    .trim_start_matches("## user")
                    .trim()
                    .replace("{errors}", "invalid article JSON structure")
                    .replace(
                        "{shape}",
                        r#"{"title":string,"body":string,"names":string[]}"#,
                    );
                let repaired = model
                    .complete(
                        &input.provider,
                        CompletionInput {
                            system: system.clone(),
                            user: format!("{user}\n\n{repair}"),
                            json: true,
                        },
                        call(calls, "repair"),
                    )
                    .await?;
                ArticleDraft::parse(&repaired)?
            }
        };
        let names = draft.names.iter().map(|s| s.to_lowercase()).collect();
        let analysis = analyze(&draft.body, &input.known, &exposed, &control, &names);
        let below = analysis
            .coverage
            .is_none_or(|coverage| coverage < input.tier.minimum_coverage());
        let mut diagnostics = Vec::new();
        if below {
            diagnostics.push("实际覆盖率低于所选档位或不可计算".into());
        }
        if analysis.token_count < u64::from(settings.article_min_words)
            || analysis.token_count > u64::from(settings.article_max_words)
        {
            diagnostics.push("文章长度不在设置范围内".into());
        }
        for word in &exposed {
            if analysis.occurrences.get(word).copied().unwrap_or(0) < 2 {
                diagnostics.push(format!("目标词出现不足：{word}"));
            }
        }
        for word in &control {
            if analysis.occurrences.get(word).copied().unwrap_or(0) > 0 {
                diagnostics.push(format!("对照词偶然出现：{word}"));
            }
        }
        if !below || attempt == settings.generation_max_retries {
            return Ok(GeneratedArticle {
                draft,
                analysis,
                genre,
                underlying_calls: calls,
                diagnostics,
            });
        }
        avoid = analysis
            .unknown
            .keys()
            .filter(|word| !control.contains(*word) && !exposed.contains(*word))
            .cloned()
            .collect();
    }
    unreachable!("bounded generation always returns on its last attempt")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cijian_domain::{
        provider::{ProviderConfig, ProviderKind},
        settings::Settings,
    };
    use std::sync::Mutex;
    struct Model {
        outputs: Mutex<std::collections::VecDeque<String>>,
        prompts: Mutex<Vec<String>>,
    }
    impl LanguageModel for Model {
        async fn complete(
            &self,
            _: &ProviderSnapshot,
            input: CompletionInput,
            _: CallStart,
        ) -> Result<String> {
            self.prompts.lock().unwrap().push(input.user);
            self.outputs
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(AppError::new(ErrorCode::ProviderError, "fixture exhausted"))
        }
    }
    fn input() -> GenerationInput {
        GenerationInput {
            profile: "p".into(),
            operation: "op".into(),
            now: 0,
            settings: Settings::default().try_into().unwrap(),
            tier: Tier::Easy,
            genre: Genre::Auto,
            last_auto_genre: Some(Genre::Story),
            targets: Targets {
                exposed: vec!["cat".into()],
                control: vec!["saffron".into()],
            },
            known: BTreeSet::new(),
            provider: ProviderSnapshot {
                config: ProviderConfig {
                    kind: ProviderKind::Llm,
                    endpoint: "https://example.invalid/v1".into(),
                    model: "configured".into(),
                    voice: None,
                    revision: 1,
                },
                secret: None,
            },
        }
    }
    #[tokio::test]
    async fn repair_rewrite_budget_and_no_control_leak() {
        let valid =
            r#"{"title":"Original","body":"cat cat saffron mysterious enigmatic","names":[]}"#
                .to_owned();
        let model = Model {
            outputs: Mutex::new(
                vec![
                    "bad".into(),
                    valid.clone(),
                    "bad".into(),
                    valid.clone(),
                    "bad".into(),
                    valid,
                ]
                .into(),
            ),
            prompts: Mutex::new(Vec::new()),
        };
        let result = generate(&model, &input()).await.unwrap();
        assert_eq!(result.underlying_calls, 6);
        assert_eq!(result.genre, Genre::Dialogue);
        assert!(!result.diagnostics.is_empty());
        assert!(
            model
                .prompts
                .lock()
                .unwrap()
                .iter()
                .all(|p| !p.contains("saffron"))
        );
    }
    #[tokio::test]
    async fn failed_repair_stops_without_pseudo_article() {
        let model = Model {
            outputs: Mutex::new(vec!["bad".into(), "bad".into()].into()),
            prompts: Mutex::new(Vec::new()),
        };
        assert_eq!(
            generate(&model, &input()).await.unwrap_err().code,
            ErrorCode::LlmOutputInvalid
        );
        assert_eq!(model.prompts.lock().unwrap().len(), 2);
    }
}
