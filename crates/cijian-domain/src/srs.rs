use crate::{CardId, ValidationError};
use fsrs::{FSRS, MemoryState};
use serde::{Deserialize, Serialize};

pub const SCHEDULER_VERSION: &str = "fsrs-rs/6.6.2;ts-fsrs/5.4.2-steps;v1";
const DAY: i64 = 86_400_000;
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
        Ok(())
    }
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
    let mut days =
        std::array::from_fn::<_, 4, _>(|i| states[i].interval.round().clamp(1.0, 36500.0) as u32);
    if card.state == State::Review {
        days[1] = days[1].min(days[2]);
        days[2] = days[2].max(days[1] + 1);
        days[3] = days[3].max(days[2] + 1);
    }
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
        let step = match (card.state, rating) {
            (State::Review, Rating::Again) | (State::Relearning, Rating::Again) => Some((10, 0)),
            (State::Relearning, Rating::Hard) => Some((15, 0)),
            (State::New | State::Learning, Rating::Again) => Some((1, 0)),
            (State::New | State::Learning, Rating::Hard) => Some((6, card.learning_steps)),
            (State::New | State::Learning, Rating::Good) if card.learning_steps == 0 => {
                Some((10, 1))
            }
            _ => None,
        };
        let interval = if let Some((minutes, step)) = step {
            out.state = if matches!(card.state, State::Review | State::Relearning) {
                State::Relearning
            } else {
                State::Learning
            };
            out.learning_steps = step;
            out.scheduled_days = 0;
            minutes * 60000
        } else {
            out.state = State::Review;
            out.learning_steps = 0;
            out.scheduled_days = days[index];
            i64::from(days[index]) * DAY
        };
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
    if card.due > now {
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
    let mut eligible: Vec<_> = cards
        .iter()
        .filter(|c| !c.suspended && !c.archived && c.srs.due <= now)
        .collect();
    eligible.sort_by_key(|c| {
        (
            match c.srs.state {
                State::Learning | State::Relearning => 0,
                State::Review => 1,
                State::New => 2,
            },
            c.srs.due,
            c.created_at,
            c.id.as_str(),
        )
    });
    let (mut new, mut review) = (quota.new_remaining, quota.review_remaining);
    eligible
        .into_iter()
        .filter_map(|c| {
            let keep = match c.srs.state {
                State::New => {
                    if new > 0 {
                        new -= 1;
                        true
                    } else {
                        false
                    }
                }
                State::Review => {
                    if review > 0 {
                        review -= 1;
                        true
                    } else {
                        false
                    }
                }
                _ => true,
            };
            keep.then(|| c.id.clone())
        })
        .collect()
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
