use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StudyCounts {
    pub new_cards: u64,
    pub reviews: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityDay {
    pub date: String,
    pub count: u64,
    pub future: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoveragePoint {
    pub article: String,
    pub title: String,
    pub created_at: i64,
    pub tier: cijian_domain::settings::Tier,
    pub coverage: Option<f64>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExperimentGroup {
    pub sample: u64,
    pub again: u64,
    pub unreviewed: u64,
    pub excluded: u64,
    pub again_rate: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentReport {
    pub version: String,
    pub exposed: ExperimentGroup,
    pub control: ExperimentGroup,
    pub difference_percentage_points: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedWord {
    pub lemma: String,
    pub archived: bool,
    pub retrievability: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub today: StudyCounts,
    pub week: StudyCounts,
    pub reading_completion: Option<f64>,
    /// Shares of the last feedback per article and mode. Each is `None` until
    /// at least one reading or listening feedback exists, so the page can show
    /// a measurement instead of a hardcoded dash.
    pub feedback_too_hard: Option<f64>,
    pub feedback_just_right: Option<f64>,
    pub feedback_too_easy: Option<f64>,
    pub heatmap: Vec<ActivityDay>,
    pub coverage: Vec<CoveragePoint>,
    pub unknown_median_all: Option<f64>,
    pub unknown_median_week: Option<f64>,
    pub learned: Vec<LearnedWord>,
    pub learned_total: u64,
    pub experiment: ExperimentReport,
}
