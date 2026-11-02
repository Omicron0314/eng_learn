use crate::{
    RandomSource, ValidationError,
    srs::{SrsState, State, retrievability},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone)]
pub struct KnowledgeCard {
    pub lemma: String,
    pub srs: SrsState,
    pub archived: bool,
}
#[derive(Debug, Clone)]
pub struct Mark {
    pub known: bool,
    pub at: i64,
}
pub fn known_set(
    baseline: &BTreeSet<String>,
    cards: &[KnowledgeCard],
    marks: &BTreeMap<String, Mark>,
    exposed: &BTreeSet<String>,
    now: i64,
) -> Result<BTreeSet<String>, ValidationError> {
    let mut known = baseline.union(exposed).cloned().collect::<BTreeSet<_>>();
    for card in cards {
        if !card.archived
            && card.srs.state == State::Review
            && retrievability(&card.srs, now)? >= 0.8
        {
            known.insert(card.lemma.clone());
        }
    }
    for (lemma, mark) in marks {
        if mark.at > now {
            continue;
        }
        if mark.known {
            known.insert(lemma.clone());
        } else {
            let later_review = cards.iter().any(|c| {
                c.lemma == *lemma
                    && c.srs
                        .last_review
                        .is_some_and(|at| at > mark.at && at <= now)
            });
            if !later_review {
                known.remove(lemma);
            }
        }
    }
    Ok(known)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    AgainOrHard,
    FirstLearned,
    DueWeak,
}
#[derive(Debug, Clone)]
pub struct Candidate {
    pub lemma: String,
    pub reason: Reason,
    pub last_activity: i64,
    pub recent_occurrences: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Targets {
    pub exposed: Vec<String>,
    pub control: Vec<String>,
}
pub fn select_targets(
    candidates: &[Candidate],
    count: u32,
    ratio: f64,
    rng: &mut impl RandomSource,
) -> Result<Targets, ValidationError> {
    if !(1..=12).contains(&count) || !ratio.is_finite() || !(0.0..=0.5).contains(&ratio) {
        return Err(ValidationError::new("targets", "目标词参数无效"));
    }
    let mut sorted = candidates
        .iter()
        .filter(|c| c.recent_occurrences < 3 && !c.lemma.contains(' '))
        .collect::<Vec<_>>();
    sorted.sort_by(|a, b| {
        (a.reason, a.last_activity, &a.lemma).cmp(&(b.reason, b.last_activity, &b.lemma))
    });
    let mut seen = BTreeSet::new();
    let mut selected = sorted
        .into_iter()
        .filter(|c| seen.insert(c.lemma.clone()))
        .take(count as usize)
        .map(|c| c.lemma.clone())
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(ValidationError::new("targets", "NO_TARGETS"));
    }
    for i in (1..selected.len()).rev() {
        let j = (rng.next_u64() % ((i + 1) as u64)) as usize;
        selected.swap(i, j);
    }
    let n = ((selected.len() as f64 * ratio).floor() as usize).min(selected.len() - 1);
    let exposed = selected.split_off(n);
    Ok(Targets {
        exposed,
        control: selected,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixed(u64);
    impl RandomSource for Fixed {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
            self.0
        }
    }
    #[test]
    fn unknown_mark_excludes_baseline_until_later_review() {
        let baseline = BTreeSet::from(["cat".into()]);
        let marks = BTreeMap::from([(
            "cat".into(),
            Mark {
                known: false,
                at: 10,
            },
        )]);
        assert!(
            known_set(&baseline, &[], &marks, &BTreeSet::new(), 20)
                .unwrap()
                .is_empty()
        );
        let mut srs = SrsState::new(0);
        srs.last_review = Some(11);
        srs.state = State::Review;
        srs.reps = 1;
        srs.stability = 10.;
        srs.difficulty = 5.;
        let cards = [KnowledgeCard {
            lemma: "cat".into(),
            srs,
            archived: false,
        }];
        assert!(
            known_set(&baseline, &cards, &marks, &BTreeSet::new(), 20)
                .unwrap()
                .contains("cat")
        );
    }
    #[test]
    fn stable_selection_deduplicates_and_never_only_controls() {
        let candidates = (0..8)
            .map(|i| Candidate {
                lemma: format!("word{i}"),
                reason: Reason::FirstLearned,
                last_activity: i,
                recent_occurrences: 0,
            })
            .collect::<Vec<_>>();
        let a = select_targets(&candidates, 8, 0.2, &mut Fixed(4)).unwrap();
        assert_eq!(
            a,
            select_targets(&candidates, 8, 0.2, &mut Fixed(4)).unwrap()
        );
        assert_eq!(a.control.len(), 1);
        assert_eq!(a.exposed.len(), 7);
        assert_eq!(
            select_targets(&candidates, 1, 0.5, &mut Fixed(1))
                .unwrap()
                .control
                .len(),
            0
        );
        assert!(select_targets(&[], 8, 0.2, &mut Fixed(1)).is_err());
    }
}
