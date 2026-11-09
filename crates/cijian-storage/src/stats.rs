#[cfg(test)]
mod tests;
use super::*;
use cijian_domain::{
    srs::retrievability,
    time::{heatmap_days, study_day, study_week_start},
};
use cijian_ports::stats::*;
use std::collections::{BTreeMap, BTreeSet};
fn ratio(n: u64, d: u64) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}
fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.
    } else {
        values[middle]
    })
}
fn counts(db: &Connection, profile: &str, start: i64, now: i64) -> Result<StudyCounts> {
    let new_cards:i64=db.query_row("SELECT count(DISTINCT card_id) FROM review_logs WHERE profile_id=?1 AND reviewed_at>=?2 AND reviewed_at<=?3 AND state_before='new'",params![profile,start,now],|r|r.get(0)).map_err(unavailable)?;
    let reviews:i64=db.query_row("SELECT count(*) FROM review_logs WHERE profile_id=?1 AND reviewed_at>=?2 AND reviewed_at<=?3 AND state_before<>'new'",params![profile,start,now],|r|r.get(0)).map_err(unavailable)?;
    Ok(StudyCounts {
        new_cards: new_cards as u64,
        reviews: reviews as u64,
    })
}
impl Store {
    pub fn experiment_report(
        &self,
        profile: &str,
        now: i64,
        version: &str,
    ) -> Result<ExperimentReport> {
        let db = self.connection.lock().map_err(unavailable)?;
        let mut statement=db.prepare("SELECT t.role,t.occurrences,(SELECT r.rating FROM review_logs r JOIN cards c ON c.id=r.card_id AND c.profile_id=r.profile_id WHERE r.profile_id=a.profile_id AND c.lexeme_id=t.lexeme_id AND r.reviewed_at>=a.created_at AND r.reviewed_at<a.created_at+604800000 AND r.reviewed_at<=?2 ORDER BY r.reviewed_at,r.id LIMIT 1) FROM article_targets t JOIN articles a ON a.id=t.article_id AND a.profile_id=t.profile_id WHERE a.profile_id=?1 AND a.created_at<=?2 AND a.experiment_version=?3").map_err(unavailable)?;
        let mut report = ExperimentReport {
            version: version.into(),
            exposed: Default::default(),
            control: Default::default(),
            difference_percentage_points: None,
        };
        let rows = statement
            .query_map(params![profile, now, version], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(unavailable)?;
        for row in rows {
            let (role, occurrences, rating) = row.map_err(unavailable)?;
            let group = if role == "exposed" {
                &mut report.exposed
            } else {
                &mut report.control
            };
            if (role == "exposed" && occurrences == 0) || (role == "control" && occurrences > 0) {
                group.excluded += 1;
            } else if let Some(rating) = rating {
                group.sample += 1;
                group.again += u64::from(rating == "again");
            } else {
                group.unreviewed += 1;
            }
        }
        report.exposed.again_rate = ratio(report.exposed.again, report.exposed.sample);
        report.control.again_rate = ratio(report.control.again, report.control.sample);
        report.difference_percentage_points = report
            .exposed
            .again_rate
            .zip(report.control.again_rate)
            .map(|(a, b)| (a - b) * 100.);
        Ok(report)
    }
    pub fn stats(&self, profile: &str, now: i64, page: u32) -> Result<StatsSnapshot> {
        let (settings, _) = self.settings(profile)?;
        let zone = settings.timezone.parse().map_err(unavailable)?;
        let day = study_day(now, zone, settings.day_start_hour).map_err(unavailable)?;
        let week_start =
            study_week_start(now, zone, settings.day_start_hour).map_err(unavailable)?;
        let days = heatmap_days(now, zone, settings.day_start_hour).map_err(unavailable)?;
        let mut recent_start = day.start_ms;
        for _ in 0..6 {
            recent_start = study_day(recent_start - 1, zone, settings.day_start_hour)
                .map_err(unavailable)?
                .start_ms;
        }
        let experiment = self.experiment_report(profile, now, "experiment.v1")?;
        let db = self.connection.lock().map_err(unavailable)?;
        let today = counts(&db, profile, day.start_ms, now)?;
        let week = counts(&db, profile, week_start, now)?;
        let opened:i64=db.query_row("SELECT count(DISTINCT article_id) FROM reading_sessions WHERE profile_id=?1 AND opened_at<=?2",params![profile,now],|r|r.get(0)).map_err(unavailable)?;
        let completed:i64=db.query_row("SELECT count(DISTINCT article_id) FROM reading_sessions WHERE profile_id=?1 AND opened_at<=?2 AND completed_at<=?2",params![profile,now],|r|r.get(0)).map_err(unavailable)?;
        let mut statement=db.prepare("SELECT payload_json FROM events WHERE profile_id=?1 AND type='difficulty_feedback' AND occurred_at<=?2 ORDER BY rowid").map_err(unavailable)?;
        let mut feedback = BTreeMap::new();
        for row in statement
            .query_map(params![profile, now], |r| r.get::<_, String>(0))
            .map_err(unavailable)?
        {
            let value: serde_json::Value =
                serde_json::from_str(&row.map_err(unavailable)?).map_err(unavailable)?;
            if let (Some(article), Some(mode), Some(value)) = (
                value["articleId"].as_str(),
                value["mode"].as_str(),
                value["value"].as_str(),
            ) {
                feedback.insert((article.to_owned(), mode.to_owned()), value.to_owned());
            }
        }
        let feedback_just_right = ratio(
            feedback
                .values()
                .filter(|v| ["justRight", "just_right"].contains(&v.as_str()))
                .count() as u64,
            feedback.len() as u64,
        );
        let mut daily = BTreeMap::<String, u64>::new();
        let mut statement=db.prepare("SELECT reviewed_at FROM review_logs WHERE profile_id=?1 AND reviewed_at>=?2 AND reviewed_at<=?3").map_err(unavailable)?;
        for row in statement
            .query_map(params![profile, days[0].start_ms, now], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(unavailable)?
        {
            let date = study_day(row.map_err(unavailable)?, zone, settings.day_start_hour)
                .map_err(unavailable)?
                .date
                .to_string();
            *daily.entry(date).or_default() += 1;
        }
        for table in ["reading_sessions", "audio_sessions"] {
            let mut seen = BTreeSet::new();
            let mut statement=db.prepare(&format!("SELECT article_id,completed_at FROM {table} WHERE profile_id=?1 AND completed_at>=?2 AND completed_at<=?3")).map_err(unavailable)?;
            for row in statement
                .query_map(params![profile, days[0].start_ms, now], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                })
                .map_err(unavailable)?
            {
                let (article, at) = row.map_err(unavailable)?;
                let date = study_day(at, zone, settings.day_start_hour)
                    .map_err(unavailable)?
                    .date
                    .to_string();
                if seen.insert((article, date.clone())) {
                    *daily.entry(date).or_default() += 1;
                }
            }
        }
        let heatmap = days
            .into_iter()
            .map(|d| ActivityDay {
                count: *daily.get(&d.date.to_string()).unwrap_or(&0),
                date: d.date.to_string(),
                future: d.date > day.date,
            })
            .collect();
        let mut coverage = Vec::new();
        let mut all = Vec::new();
        let mut recent = Vec::new();
        let mut statement=db.prepare("SELECT id,title,created_at,tier,analysis_json FROM articles WHERE profile_id=?1 AND created_at<=?2 ORDER BY created_at,id").map_err(unavailable)?;
        let rows = statement
            .query_map(params![profile, now], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(unavailable)?;
        for row in rows {
            let (article, title, created_at, tier, analysis) = row.map_err(unavailable)?;
            let value: serde_json::Value = serde_json::from_str(&analysis).map_err(unavailable)?;
            let coverage_value = value["analysis"]["coverage"]
                .as_f64()
                .filter(|c| c.is_finite() && (0.0..=1.).contains(c));
            if let Some(value) = coverage_value {
                all.push(1. - value);
                if created_at >= recent_start {
                    recent.push(1. - value);
                }
            }
            coverage.push(CoveragePoint {
                article,
                title,
                created_at,
                tier: serde_json::from_str(&tier).map_err(unavailable)?,
                coverage: coverage_value,
            });
        }
        let learned_total:i64=db.query_row("SELECT count(DISTINCT c.lexeme_id) FROM cards c JOIN review_logs r ON r.card_id=c.id AND r.profile_id=c.profile_id WHERE c.profile_id=?1 AND r.reviewed_at<=?2",params![profile,now],|r|r.get(0)).map_err(unavailable)?;
        let mut statement=db.prepare("SELECT c.*,l.lemma FROM card_view c JOIN lexemes l ON l.id=c.lexeme_id WHERE c.profile_id=?1 AND EXISTS(SELECT 1 FROM review_logs r WHERE r.card_id=c.id AND r.profile_id=c.profile_id AND r.reviewed_at<=?2) ORDER BY l.lemma,c.id LIMIT 50 OFFSET ?3").map_err(unavailable)?;
        let learned = statement
            .query_map(params![profile, now, i64::from(page) * 50], |r| {
                let card = super::library::read_card(r)?;
                Ok(LearnedWord {
                    lemma: r.get("lemma")?,
                    archived: card.archived,
                    retrievability: retrievability(&card.srs, now).ok(),
                })
            })
            .map_err(unavailable)?
            .collect::<std::result::Result<_, _>>()
            .map_err(unavailable)?;
        Ok(StatsSnapshot {
            today,
            week,
            reading_completion: ratio(completed as u64, opened as u64),
            feedback_just_right,
            heatmap,
            coverage,
            unknown_median_all: median(all),
            unknown_median_week: median(recent),
            learned,
            learned_total: learned_total as u64,
            experiment,
        })
    }
}
