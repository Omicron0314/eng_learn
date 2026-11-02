use cijian_domain::{gloss::Gloss, reading::Selection};
use cijian_ports::{
    AppError, ErrorCode, Result,
    provider::{CallStart, CompletionInput, LanguageModel, ProviderSnapshot},
};
const PROMPT: &str = include_str!("../../../prompts/gloss.v1.md");
pub async fn explain(
    model: &impl LanguageModel,
    snapshot: &ProviderSnapshot,
    selection: &Selection,
    call: CallStart,
) -> Result<Gloss> {
    let (system, template) = PROMPT
        .split_once("## user")
        .expect("versioned gloss sections");
    let system = system.trim_start_matches("## system").trim().to_owned();
    let user = template
        .trim()
        .replace("{surface}", &selection.surface)
        .replace("{lemma}", &selection.lemma)
        .replace("{sentence}", &selection.sentence);
    let output = model
        .complete(
            snapshot,
            CompletionInput {
                system: system.clone(),
                user: user.clone(),
                json: true,
            },
            call.clone(),
        )
        .await?;
    match Gloss::parse(&output, &selection.lemma) {
        Ok(gloss) => Ok(gloss),
        Err(_) => {
            let mut repair = call;
            repair.call_id.push_str("-repair");
            repair.purpose = "gloss-repair".into();
            repair.prompt_version = "json-repair.v1".into();
            let repair_prompt=include_str!("../../../prompts/json-repair.v1.md").trim_start_matches("## user").trim().replace("{errors}","Invalid gloss JSON or field constraints").replace("{shape}",r#"{"lemma":string,"pos":string,"meaningZh":string,"meaningEn":string,"ipa":string}"#);
            let output = model
                .complete(
                    snapshot,
                    CompletionInput {
                        system,
                        user: format!("{user}\n\n{repair_prompt}"),
                        json: true,
                    },
                    repair,
                )
                .await?;
            Gloss::parse(&output, &selection.lemma)
                .map_err(|e| AppError::new(ErrorCode::LlmOutputInvalid, e.to_string()))
        }
    }
}
