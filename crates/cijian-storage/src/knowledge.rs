use super::*;
use cijian_domain::{
    knowledge::{Candidate, KnowledgeCard, Mark, Reason, known_set},
    settings::Genre,
    srs::{State, retrievability},
    time::study_day,
};
use std::collections::{BTreeMap, BTreeSet};
pub struct GenerationMaterial {
    pub known: BTreeSet<String>,
    pub candidates: Vec<Candidate>,
    pub lexeme_ids: BTreeMap<String, String>,
    pub last_auto_genre: Option<Genre>,
}
impl Store {
    pub fn generation_material(&self, profile: &str, now: i64) -> Result<GenerationMaterial> {
        let (settings, _) = self.settings(profile)?;
        let timezone = settings.timezone.parse().map_err(unavailable)?;
        let day = study_day(now, timezone, settings.day_start_hour).map_err(unavailable)?;
        let previous =
            study_day(day.start_ms - 1, timezone, settings.day_start_hour).map_err(unavailable)?;
        let third = study_day(previous.start_ms - 1, timezone, settings.day_start_hour)
            .map_err(unavailable)?;
        let db = self.connection.lock().map_err(unavailable)?;
        let mut statement=db.prepare("SELECT DISTINCT l.lemma FROM lexemes l JOIN book_entries e ON e.lexeme_id=l.id JOIN word_books b ON b.id=e.book_id WHERE e.profile_id=?1 AND b.kind='baseline' AND b.archived_at IS NULL").map_err(unavailable)?;
        let baseline = statement
            .query_map([profile], |r| r.get(0))
            .map_err(unavailable)?
            .collect::<std::result::Result<BTreeSet<String>, _>>()
            .map_err(unavailable)?;
        let mut statement=db.prepare("SELECT l.lemma,m.status,m.marked_at FROM lexeme_marks m JOIN lexemes l ON l.id=m.lexeme_id WHERE m.profile_id=?1").map_err(unavailable)?;
        let marks = statement
            .query_map([profile], |r| {
                let status: String = r.get(1)?;
                Ok((
                    r.get::<_, String>(0)?,
                    Mark {
                        known: status == "known",
                        at: r.get(2)?,
                    },
                ))
            })
            .map_err(unavailable)?
            .collect::<std::result::Result<BTreeMap<_, _>, _>>()
            .map_err(unavailable)?;
        let mut statement=db.prepare("SELECT c.*,l.lemma,l.kind AS lexeme_kind FROM cards c JOIN lexemes l ON l.id=c.lexeme_id WHERE c.profile_id=?1").map_err(unavailable)?;
        let cards = statement
            .query_map([profile], |r| {
                Ok((
                    super::library::read_card(r)?,
                    r.get::<_, String>("lemma")?,
                    r.get::<_, String>("lexeme_kind")?,
                ))
            })
            .map_err(unavailable)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(unavailable)?;
        let known_cards = cards
            .iter()
            .map(|(c, lemma, _)| KnowledgeCard {
                lemma: lemma.clone(),
                srs: c.srs.clone(),
                archived: c.archived,
            })
            .collect::<Vec<_>>();
        let known = known_set(&baseline, &known_cards, &marks, &BTreeSet::new(), now)
            .map_err(unavailable)?;
        let mut candidates = Vec::new();
        let mut lexeme_ids = BTreeMap::new();
        for (card, lemma, kind) in cards {
            if card.suspended || card.archived || kind != "word" {
                continue;
            }
            let hard:Option<i64>=db.query_row("SELECT min(reviewed_at) FROM review_logs WHERE profile_id=?1 AND card_id=?2 AND reviewed_at>=?3 AND reviewed_at<?4 AND rating IN ('again','hard')",params![profile,card.id,day.start_ms,day.end_ms],|r|r.get(0)).map_err(unavailable)?;
            let first:Option<i64>=db.query_row("SELECT min(reviewed_at) FROM review_logs WHERE profile_id=?1 AND card_id=?2 AND state_before='new' AND reviewed_at>=?3 AND reviewed_at<?4",params![profile,card.id,day.start_ms,day.end_ms],|r|r.get(0)).map_err(unavailable)?;
            let reason = if let Some(at) = hard {
                Some((Reason::AgainOrHard, at))
            } else if let Some(at) = first {
                Some((Reason::FirstLearned, at))
            } else if card.srs.state != State::New
                && card.srs.due <= now
                && retrievability(&card.srs, now).map_err(unavailable)? < 0.8
            {
                Some((Reason::DueWeak, card.srs.due))
            } else {
                None
            };
            if let Some((reason, last_activity)) = reason {
                let recent:i64=db.query_row("SELECT COALESCE(sum(t.occurrences),0) FROM article_targets t JOIN articles a ON a.id=t.article_id WHERE t.profile_id=?1 AND t.lexeme_id=?2 AND a.created_at>=?3 AND a.created_at<?4",params![profile,card.lexeme_id,third.start_ms,day.end_ms],|r|r.get(0)).map_err(unavailable)?;
                candidates.push(Candidate {
                    lemma: lemma.clone(),
                    reason,
                    last_activity,
                    recent_occurrences: u32::try_from(recent).unwrap_or(u32::MAX),
                });
                lexeme_ids.insert(lemma, card.lexeme_id);
            }
        }
        let last:Option<String>=db.query_row("SELECT a.genre FROM articles a JOIN jobs j ON j.result_ref=a.id JOIN generation_snapshots s ON s.job_id=j.id WHERE a.profile_id=?1 AND j.status='succeeded' AND json_extract(s.snapshot_json,'$.genre')='auto' ORDER BY a.created_at DESC,a.id DESC LIMIT 1",[profile],|r|r.get(0)).optional().map_err(unavailable)?;
        let last_auto_genre = last
            .map(|s| serde_json::from_str(&s).map_err(unavailable))
            .transpose()?;
        Ok(GenerationMaterial {
            known,
            candidates,
            lexeme_ids,
            last_auto_genre,
        })
    }
}
