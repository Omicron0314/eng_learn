use crate::{CardId, ValidationError};
use fsrs::{FSRS, MemoryState};
use serde::{Deserialize, Serialize};

pub const SCHEDULER_VERSION: &str = "fsrs-rs/6.6.2;anki-26.09.3-ease;v3";
const DAY: i64 = 86_400_000;
const MINUTE: i64 = 60_000;
/// Anki's default learning steps: 1m, 10m.
const LEARN_STEP_MINUTES: [i64; 2] = [1, 10];
/// Anki's default relearning step: 10m.
const RELEARN_STEP_MINUTES: i64 = 10;
/// Anki's default learn-ahead limit: 20 minutes.
pub const LEARN_AHEAD_MS: i64 = 20 * MINUTE;
/// Anki's default graduating intervals.
const GRADUATING_GOOD_DAYS: u32 = 1;
const GRADUATING_EASY_DAYS: u32 = 4;
/// Anki's non-FSRS review scheduling defaults.
pub const INITIAL_EASE_FACTOR: u32 = 2500;
const MINIMUM_EASE_FACTOR: u32 = 1300;
const EASE_AGAIN_DELTA: i32 = -200;
const EASE_HARD_DELTA: i32 = -150;
const EASE_EASY_DELTA: i32 = 150;
const HARD_MULTIPLIER: f64 = 1.2;
const EASY_MULTIPLIER: f64 = 1.3;
const MAX_INTERVAL_DAYS: u32 = 36_500;
const LAPSE_MULTIPLIER: f64 = 0.0;
const MINIMUM_LAPSE_INTERVAL: u32 = 1;
const PARAMETERS: [f32; 21] = [
    0.212, 1.2931, 2.3065, 8.2956, 6.4133, 0.8334, 3.0194, 0.001, 1.8722, 0.1666, 0.796, 1.4835,
    0.0614, 0.2629, 1.6483, 0.6014, 1.8729, 0.5425, 0.0912, 0.0658, 0.1542,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    New,
    Learning,
    Review,
    Relearning,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Again,
    Hard,
    Good,
    Easy,
}
impl Rating {
    pub const ALL: [Self; 4] = [Self::Again, Self::Hard, Self::Good, Self::Easy];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SrsState {
    pub state: State,
    pub due: i64,
    pub stability: f64,
    pub difficulty: f64,
    pub reps: u32,
    pub lapses: u32,
    pub last_review: Option<i64>,
    pub elapsed_days: u32,
    pub scheduled_days: u32,
    pub learning_steps: u32,
    #[serde(default = "initial_ease_factor")]
    pub ease_factor: u32,
}
impl SrsState {
    pub fn new(now: i64) -> Self {
        Self {
            state: State::New,
            due: now,
            stability: 0.0,
            difficulty: 0.0,
            reps: 0,
            lapses: 0,
            last_review: None,
            elapsed_days: 0,
            scheduled_days: 0,
            learning_steps: 0,
            ease_factor: INITIAL_EASE_FACTOR,
        }
    }
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !self.stability.is_finite()
            || !self.difficulty.is_finite()
            || self.stability < 0.0
            || !(0.0..=10.0).contains(&self.difficulty)
            || self.lapses > self.reps
        {
            return Err(ValidationError::new("srs", "调度状态无效"));
        }
        if self.state != State::New
            && (self.last_review.is_none()
                || self.stability <= 0.0
                || self.difficulty < 1.0
                || self.reps == 0)
        {
            return Err(ValidationError::new("srs", "旧状态缺少必要记忆字段"));
        }
        if self.learning_steps > 1 || (self.state == State::Relearning && self.learning_steps != 0)
        {
            return Err(ValidationError::new("learningSteps", "无法映射旧学习步骤"));
        }
        if self.ease_factor < MINIMUM_EASE_FACTOR {
            return Err(ValidationError::new("ease", "易度因子无效"));
        }
        Ok(())
    }
}

fn initial_ease_factor() -> u32 {
    INITIAL_EASE_FACTOR
}

fn adjust_ease(ease: u32, delta: i32) -> u32 {
    if delta >= 0 {
        ease.saturating_add(delta as u32)
    } else {
        ease.saturating_sub(delta.unsigned_abs())
            .max(MINIMUM_EASE_FACTOR)
    }
}

fn clamp_interval(days: f64) -> u32 {
    days.round().clamp(1.0, f64::from(MAX_INTERVAL_DAYS)) as u32
}

/// Anki's non-FSRS hard/good/easy interval calculation, without fuzz.
fn review_intervals(card: &SrsState, elapsed: u32) -> (u32, u32, u32) {
    let scheduled = f64::from(card.scheduled_days.max(1));
    let elapsed = f64::from(elapsed);
    let ease = f64::from(card.ease_factor) / 1000.0;
    let (hard, good, easy) = if elapsed < scheduled {
        let hard = (elapsed * HARD_MULTIPLIER).max(scheduled * HARD_MULTIPLIER / 2.0);
        let good = (elapsed * ease).max(scheduled);
        let reduced_bonus = EASY_MULTIPLIER - (EASY_MULTIPLIER - 1.0) / 2.0;
        (hard, good, good * reduced_bonus)
    } else {
        let hard = (scheduled * HARD_MULTIPLIER).max(scheduled + 1.0);
        let good = ((scheduled + (elapsed - scheduled) / 2.0) * ease).max(hard + 1.0);
        let easy = ((scheduled + (elapsed - scheduled)) * ease * EASY_MULTIPLIER).max(good + 1.0);
        (hard, good, easy)
    };
    (
        clamp_interval(hard),
        clamp_interval(good),
        clamp_interval(easy),
    )
}

fn lapsed_interval(card: &SrsState) -> u32 {
    (f64::from(card.scheduled_days.max(1)) * LAPSE_MULTIPLIER)
        .max(f64::from(MINIMUM_LAPSE_INTERVAL))
        .round() as u32
}

pub fn preview(
    card: &SrsState,
    now: i64,
    retention: f64,
) -> Result<[SrsState; 4], ValidationError> {
    card.validate()?;
    if !retention.is_finite() || !(0.7..=0.99).contains(&retention) {
        return Err(ValidationError::new("requestRetention", "保留率无效"));
    }
    let elapsed = match card.last_review {
        Some(last) if card.state != State::New => u32::try_from(
            now.checked_sub(last)
                .ok_or(ValidationError::new("now", "时间超出范围"))?
                / DAY,
        )
        .map_err(|_| ValidationError::new("now", "评分早于上次复习"))?,
        _ => 0,
    };
    if card.last_review.is_some_and(|last| now < last) {
        return Err(ValidationError::new("now", "评分早于上次复习"));
    }
    let memory = (card.state != State::New).then_some(MemoryState {
        stability: card.stability as f32,
        difficulty: card.difficulty as f32,
    });
    let model = FSRS::new(&PARAMETERS).map_err(|_| ValidationError::new("srs", "调度参数无效"))?;
    let next = model
        .next_states(memory, retention as f32, elapsed)
        .map_err(|_| ValidationError::new("srs", "无法计算记忆状态"))?;
    let states = [next.again, next.hard, next.good, next.easy];
    let (hard_days, good_days, easy_days) = review_intervals(card, elapsed);
    let lapsed_days = lapsed_interval(card);
    let mut output = std::array::from_fn(|_| card.clone());
    for (index, out) in output.iter_mut().enumerate() {
        let rating = Rating::ALL[index];
        out.stability = f64::from(states[index].memory.stability);
        out.difficulty = f64::from(states[index].memory.difficulty);
        out.reps = card
            .reps
            .checked_add(1)
            .ok_or(ValidationError::new("reps", "复习次数超出范围"))?;
        out.elapsed_days = elapsed;
        out.last_review = Some(now);
        // Anki's learning-step transitions, followed by its non-FSRS ease
        // scheduling for review/relearning cards.
        let (state, interval, step, ease, scheduled_days) = match (card.state, rating) {
            (State::New | State::Learning, Rating::Again) => (
                State::Learning,
                LEARN_STEP_MINUTES[0] * MINUTE,
                0,
                card.ease_factor,
                0,
            ),
            (State::New | State::Learning, Rating::Hard) => {
                let delay = if card.learning_steps == 0 {
                    // Anki uses the average of the first two steps (5.5m).
                    (LEARN_STEP_MINUTES[0] + LEARN_STEP_MINUTES[1]) * MINUTE / 2
                } else {
                    LEARN_STEP_MINUTES[1] * MINUTE
                };
                (
                    State::Learning,
                    delay,
                    card.learning_steps,
                    card.ease_factor,
                    0,
                )
            }
            (State::New | State::Learning, Rating::Good) if card.learning_steps == 0 => (
                State::Learning,
                LEARN_STEP_MINUTES[1] * MINUTE,
                1,
                card.ease_factor,
                0,
            ),
            (State::New | State::Learning, Rating::Good) => (
                State::Review,
                i64::from(GRADUATING_GOOD_DAYS) * DAY,
                0,
                INITIAL_EASE_FACTOR,
                GRADUATING_GOOD_DAYS,
            ),
            (State::New | State::Learning, Rating::Easy) => (
                State::Review,
                i64::from(GRADUATING_EASY_DAYS) * DAY,
                0,
                INITIAL_EASE_FACTOR,
                GRADUATING_EASY_DAYS,
            ),
            (State::Review, Rating::Again) => (
                State::Relearning,
                RELEARN_STEP_MINUTES * MINUTE,
                0,
                adjust_ease(card.ease_factor, EASE_AGAIN_DELTA),
                lapsed_days,
            ),
            (State::Review, Rating::Hard) => (
                State::Review,
                i64::from(hard_days) * DAY,
                0,
                adjust_ease(card.ease_factor, EASE_HARD_DELTA),
                hard_days,
            ),
            (State::Review, Rating::Good) => (
                State::Review,
                i64::from(good_days) * DAY,
                0,
                card.ease_factor,
                good_days,
            ),
            (State::Review, Rating::Easy) => (
                State::Review,
                i64::from(easy_days) * DAY,
                0,
                adjust_ease(card.ease_factor, EASE_EASY_DELTA),
                easy_days,
            ),
            (State::Relearning, Rating::Again) => (
                State::Relearning,
                RELEARN_STEP_MINUTES * MINUTE,
                0,
                card.ease_factor,
                card.scheduled_days.max(MINIMUM_LAPSE_INTERVAL),
            ),
            (State::Relearning, Rating::Hard) => (
                State::Relearning,
                RELEARN_STEP_MINUTES * MINUTE * 3 / 2,
                0,
                card.ease_factor,
                card.scheduled_days.max(MINIMUM_LAPSE_INTERVAL),
            ),
            (State::Relearning, Rating::Good) => {
                let days = card.scheduled_days.max(MINIMUM_LAPSE_INTERVAL);
                (
                    State::Review,
                    i64::from(days) * DAY,
                    0,
                    card.ease_factor,
                    days,
                )
            }
            (State::Relearning, Rating::Easy) => {
                let days = card.scheduled_days.max(MINIMUM_LAPSE_INTERVAL) + 1;
                (
                    State::Review,
                    i64::from(days) * DAY,
                    0,
                    card.ease_factor,
                    days,
                )
            }
        };
        out.state = state;
        out.learning_steps = step;
        out.ease_factor = ease;
        out.scheduled_days = scheduled_days;
        if card.state == State::Review && rating == Rating::Again {
            out.lapses = card
                .lapses
                .checked_add(1)
                .ok_or(ValidationError::new("lapses", "遗忘次数超出范围"))?;
        }
        out.due = now
            .checked_add(interval)
            .ok_or(ValidationError::new("due", "到期时间超出范围"))?;
    }
    Ok(output)
}

pub fn rate(
    card: &SrsState,
    rating: Rating,
    now: i64,
    retention: f64,
) -> Result<SrsState, ValidationError> {
    // Anki's learn-ahead limit lets intraday learning cards be answered up to
    // 20 minutes before their scheduled due time.
    let learning_ahead = matches!(card.state, State::Learning | State::Relearning)
        && card.due <= now.saturating_add(LEARN_AHEAD_MS);
    if card.due > now && !learning_ahead {
        return Err(ValidationError::new("due", "词卡尚未到期"));
    }
    let index = Rating::ALL
        .iter()
        .position(|r| *r == rating)
        .expect("all ratings");
    Ok(preview(card, now, retention)?[index].clone())
}

pub fn retrievability(card: &SrsState, now: i64) -> Result<f64, ValidationError> {
    card.validate()?;
    if card.state == State::New {
        return Ok(0.0);
    }
    let last = card
        .last_review
        .ok_or(ValidationError::new("srs", "缺少上次复习时间"))?;
    if now < last {
        return Err(ValidationError::new("now", "时间早于上次复习"));
    }
    // Match ts-fsrs date_diff(..., "days"): completed days, not fractional days.
    let elapsed = (now
        .checked_sub(last)
        .ok_or(ValidationError::new("now", "时间超出范围"))?
        / DAY) as f32;
    Ok(f64::from(fsrs::current_retrievability(
        MemoryState {
            stability: card.stability as f32,
            difficulty: card.difficulty as f32,
        },
        elapsed,
        PARAMETERS[20],
    )))
}

#[derive(Debug, Clone)]
pub struct QueueCard {
    pub id: CardId,
    pub srs: SrsState,
    pub created_at: i64,
    pub suspended: bool,
    pub archived: bool,
}
#[derive(Debug, Clone, Copy)]
pub struct Quota {
    pub new_remaining: u32,
    pub review_remaining: u32,
}
pub fn queue(cards: &[QueueCard], now: i64, quota: Quota) -> Vec<CardId> {
    // Anki order: intraday learning due now, then the main review/new queue,
    // then learning cards within the 20-minute learn-ahead window.
    let cutoff = now.saturating_add(LEARN_AHEAD_MS);
    let mut due_learning = Vec::new();
    let mut main = Vec::new();
    let mut ahead_learning = Vec::new();
    for card in cards
        .iter()
        .filter(|c| !c.suspended && !c.archived && c.srs.due <= cutoff)
    {
        match card.srs.state {
            State::Learning | State::Relearning if card.srs.due <= now => due_learning.push(card),
            State::Learning | State::Relearning => ahead_learning.push(card),
            State::Review | State::New if card.srs.due <= now => main.push(card),
            _ => {}
        }
    }
    let learning_order = |a: &QueueCard, b: &QueueCard| {
        (a.srs.reps == 0, a.srs.due, a.created_at, a.id.as_str()).cmp(&(
            b.srs.reps == 0,
            b.srs.due,
            b.created_at,
            b.id.as_str(),
        ))
    };
    due_learning.sort_by(|a, b| learning_order(a, b));
    ahead_learning.sort_by(|a, b| learning_order(a, b));
    main.sort_by_key(|c| {
        (
            match c.srs.state {
                State::Review => 0,
                State::New => 1,
                _ => 2,
            },
            c.srs.due,
            c.created_at,
            c.id.as_str(),
        )
    });
    let (mut new, mut review) = (quota.new_remaining, quota.review_remaining);
    let mut ordered: Vec<CardId> = due_learning.into_iter().map(|c| c.id.clone()).collect();
    let mut reviews = Vec::new();
    let mut news = Vec::new();
    for card in main {
        match card.srs.state {
            State::New if new > 0 => {
                new -= 1;
                news.push(card.id.clone());
            }
            State::Review if review > 0 => {
                review -= 1;
                reviews.push(card.id.clone());
            }
            _ => {}
        }
    }
    ordered.extend(interleave(reviews, news));
    ordered.extend(ahead_learning.into_iter().map(|c| c.id.clone()));
    ordered
}

/// Anki's default MixWithReviews order for the main review/new queues.
fn interleave(reviews: Vec<CardId>, news: Vec<CardId>) -> Vec<CardId> {
    let ratio = (reviews.len() + 1) as f32 / (news.len() + 1) as f32;
    let mut output = Vec::with_capacity(reviews.len() + news.len());
    let (mut review_index, mut new_index) = (0, 0);
    while review_index < reviews.len() && new_index < news.len() {
        let relative_new = (new_index + 1) as f32 * ratio;
        if relative_new < (review_index + 1) as f32 {
            output.push(news[new_index].clone());
            new_index += 1;
        } else {
            output.push(reviews[review_index].clone());
            review_index += 1;
        }
    }
    output.extend(reviews[review_index..].iter().cloned());
    output.extend(news[new_index..].iter().cloned());
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retrievability_uses_completed_days_like_legacy() {
        let card = rate(&SrsState::new(0), Rating::Easy, 0, 0.9).unwrap();
        assert_eq!(
            retrievability(&card, 0).unwrap(),
            retrievability(&card, DAY - 1).unwrap()
        );
        assert!(retrievability(&card, DAY).unwrap() < retrievability(&card, DAY - 1).unwrap());
    }
    #[test]
    fn future_and_corrupt_states_are_rejected() {
        assert!(rate(&SrsState::new(10), Rating::Good, 9, 0.9).is_err());
        let mut card = SrsState::new(0);
        card.state = State::Review;
        assert!(preview(&card, 1, 0.9).is_err());
    }
    #[test]
    fn learning_steps_follow_anki_defaults() {
        let retention = 0.9;
        let now = 1_000_000;
        let new = SrsState::new(now);
        let previews = preview(&new, now, retention).unwrap();
        assert_eq!(previews[0].due, now + 60_000);
        assert_eq!(
            (previews[0].state, previews[0].learning_steps),
            (State::Learning, 0)
        );
        assert_eq!(previews[1].due, now + 330_000);
        assert_eq!(
            (previews[1].state, previews[1].learning_steps),
            (State::Learning, 0)
        );
        assert_eq!(previews[2].due, now + 600_000);
        assert_eq!(
            (previews[2].state, previews[2].learning_steps),
            (State::Learning, 1)
        );
        assert_eq!(previews[3].state, State::Review);
        assert_eq!(previews[3].scheduled_days, GRADUATING_EASY_DAYS);
        assert_eq!(previews[3].due, now + i64::from(GRADUATING_EASY_DAYS) * DAY);

        // Hard stays on the first step; Good advances to the last step.
        let hard = previews[1].clone();
        let after_hard = preview(&hard, hard.due, retention).unwrap();
        assert_eq!(after_hard[2].due, hard.due + 600_000);
        assert_eq!(
            (after_hard[2].state, after_hard[2].learning_steps),
            (State::Learning, 1)
        );

        // On the last step Hard repeats 10m; Good graduates to 1d.
        let last = previews[2].clone();
        let after_good = preview(&last, last.due, retention).unwrap();
        assert_eq!(after_good[1].due, last.due + 600_000);
        assert_eq!(
            (after_good[1].state, after_good[1].learning_steps),
            (State::Learning, 1)
        );
        assert_eq!(after_good[2].state, State::Review);
        assert_eq!(after_good[2].scheduled_days, GRADUATING_GOOD_DAYS);
        assert_eq!(
            after_good[2].due,
            last.due + i64::from(GRADUATING_GOOD_DAYS) * DAY
        );
        assert_eq!(after_good[3].scheduled_days, GRADUATING_EASY_DAYS);

        // Again goes back to the first step from any learning step.
        assert_eq!(after_good[0].due, last.due + 60_000);
        assert_eq!(after_good[0].learning_steps, 0);
    }
    #[test]
    fn review_intervals_follow_anki_ease_scheduler() {
        let retention = 0.9;
        let now = 1_000_000;
        let review = rate(&SrsState::new(now), Rating::Easy, now, retention).unwrap();
        assert_eq!(review.state, State::Review);
        assert_eq!(review.scheduled_days, GRADUATING_EASY_DAYS);
        assert_eq!(review.ease_factor, INITIAL_EASE_FACTOR);
        assert_eq!(review.due, now + i64::from(GRADUATING_EASY_DAYS) * DAY);

        let due = review.due;
        let previews = preview(&review, due, retention).unwrap();
        assert_eq!(previews[0].state, State::Relearning);
        assert_eq!(previews[0].due, due + RELEARN_STEP_MINUTES * MINUTE);
        assert_eq!(previews[0].scheduled_days, MINIMUM_LAPSE_INTERVAL);
        assert_eq!(previews[0].ease_factor, 2300);
        assert_eq!(previews[0].lapses, 1);
        assert_eq!(previews[1].scheduled_days, 5);
        assert_eq!(previews[1].ease_factor, 2350);
        assert_eq!(previews[2].scheduled_days, 10);
        assert_eq!(previews[2].ease_factor, 2500);
        assert_eq!(previews[3].scheduled_days, 13);
        assert_eq!(previews[3].ease_factor, 2650);

        let relearn = previews[0].clone();
        let at = relearn.due;
        let relearned = preview(&relearn, at, retention).unwrap();
        assert_eq!(relearned[0].due, at + RELEARN_STEP_MINUTES * MINUTE);
        assert_eq!(relearned[1].due, at + RELEARN_STEP_MINUTES * MINUTE * 3 / 2);
        assert_eq!(relearned[2].state, State::Review);
        assert_eq!(relearned[2].scheduled_days, 1);
        assert_eq!(relearned[2].due, at + DAY);
        assert_eq!(relearned[3].state, State::Review);
        assert_eq!(relearned[3].scheduled_days, 2);
        assert_eq!(relearned[3].due, at + 2 * DAY);
    }
    #[test]
    fn learning_card_can_be_rated_ahead_but_not_beyond_limit() {
        let retention = 0.9;
        let now = 1_000_000;
        let again = rate(&SrsState::new(now), Rating::Again, now, retention).unwrap();
        assert_eq!(again.due, now + 60_000);
        let early = rate(&again, Rating::Again, now + 1_000, retention).unwrap();
        assert_eq!(early.due, now + 61_000);
        let mut far = again.clone();
        far.due = now + LEARN_AHEAD_MS + 1;
        assert!(rate(&far, Rating::Again, now, retention).is_err());
    }
    #[test]
    fn queue_places_learn_ahead_after_main_queue() {
        let now = 1_000_000;
        let card = |id: &str, state: State, due: i64, reps: u32| {
            let mut srs = SrsState::new(0);
            srs.state = state;
            srs.due = due;
            srs.reps = reps;
            QueueCard {
                id: id.to_owned().try_into().unwrap(),
                srs,
                created_at: 0,
                suspended: false,
                archived: false,
            }
        };
        let cards = [
            card("new", State::New, now, 0),
            card("review", State::Review, now, 1),
            card("learning", State::Learning, now + 10 * MINUTE, 1),
            card("far", State::Learning, now + LEARN_AHEAD_MS + 1, 1),
        ];
        assert_eq!(
            queue(
                &cards,
                now,
                Quota {
                    new_remaining: 1,
                    review_remaining: 1
                }
            )
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
            vec!["review", "new", "learning"]
        );
        assert_eq!(
            queue(
                &cards,
                now,
                Quota {
                    new_remaining: 0,
                    review_remaining: 0
                }
            )
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
            vec!["learning"]
        );
    }
    #[test]
    fn main_queue_interleaves_reviews_and_new_cards() {
        let now = 1_000_000;
        let card = |id: &str, state: State, due: i64| {
            let mut srs = SrsState::new(0);
            srs.state = state;
            srs.due = due;
            QueueCard {
                id: id.to_owned().try_into().unwrap(),
                srs,
                created_at: 0,
                suspended: false,
                archived: false,
            }
        };
        let cards = [
            card("review-1", State::Review, now),
            card("review-2", State::Review, now),
            card("new-1", State::New, now),
        ];
        assert_eq!(
            queue(
                &cards,
                now,
                Quota {
                    new_remaining: 1,
                    review_remaining: 2
                }
            )
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
            vec!["review-1", "new-1", "review-2"]
        );
    }
    #[test]
    fn zero_limits_preserve_learning_but_exclude_archived() {
        let cards: Vec<_> = [
            State::New,
            State::Review,
            State::Learning,
            State::Relearning,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, state)| {
            let mut srs = SrsState::new(0);
            srs.state = state;
            QueueCard {
                id: i.to_string().try_into().unwrap(),
                srs,
                created_at: 0,
                suspended: false,
                archived: i == 3,
            }
        })
        .collect();
        assert_eq!(
            queue(
                &cards,
                0,
                Quota {
                    new_remaining: 0,
                    review_remaining: 0
                }
            )
            .iter()
            .map(|i| i.as_str())
            .collect::<Vec<_>>(),
            vec!["2"]
        );
    }
}
