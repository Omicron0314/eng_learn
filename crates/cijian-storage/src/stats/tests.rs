use super::*;
use crate::articles::tests::{article, request, store};
use cijian_domain::{reading::TextRange, srs::Rating};
use cijian_ports::{
    inbox::{MarkAction, MarkWord},
    library::RateCard,
    reading::ReaderCommand,
};
#[test]
fn empty_statistics_have_null_denominators_and_future_cells() {
    let s = Store::memory().unwrap();
    s.create_profile("p", "test", 0).unwrap();
    let stats = s.stats("p", 100000, 0).unwrap();
    assert_eq!(stats.heatmap.len(), 91);
    assert!(stats.heatmap.iter().any(|d| d.future));
    assert!(stats.heatmap.iter().all(|d| d.count == 0));
    assert!(stats.reading_completion.is_none());
    assert!(stats.feedback_just_right.is_none());
    assert!(stats.experiment.exposed.again_rate.is_none());
    assert!(stats.unknown_median_all.is_none());
}
#[test]
fn counts_heatmap_and_first_experiment_rating_reconcile() {
    let s = store();
    s.start_generation(request()).unwrap();
    s.claim_generation("p", "job", 0).unwrap();
    let saved = s.save_article(article()).unwrap();
    let context = |operation: &str, now| ReaderCommand {
        profile: "p".into(),
        operation: operation.into(),
        event: format!("event-{operation}"),
        article: "article".into(),
        body_hash: saved.body_hash.clone(),
        now,
    };
    let mark = s
        .mark_word(MarkWord {
            context: context("add", 2),
            range: TextRange { start: 2, end: 5 },
            action: MarkAction::AddToReview,
            expected_revision: 0,
            lexeme_id: "new-cat".into(),
            card_id: "card".into(),
            inbox_id: "inbox".into(),
            meaning: String::new(),
        })
        .unwrap();
    assert!(mark.card_created);
    let rate = |operation: &str, rating, revision, now| RateCard {
        profile_id: "p".into(),
        operation_id: operation.into(),
        event_id: format!("event-{operation}"),
        review_id: format!("review-{operation}"),
        card_id: "card".into(),
        expected_revision: revision,
        rating,
        duration_ms: 10,
        revealed: true,
        now,
    };
    s.rate(rate("first", Rating::Again, 1, 3)).unwrap();
    s.rate(rate("second", Rating::Good, 2, 60003)).unwrap();
    for (open, complete, session) in [("open1", "complete1", "one"), ("open2", "complete2", "two")]
    {
        s.open_reading(context(open, 4), session).unwrap();
        s.complete_reading(context(complete, 5), session).unwrap();
    }
    let stats = s.stats("p", 100000, 0).unwrap();
    assert_eq!(stats.today.new_cards, 1);
    assert_eq!(stats.today.reviews, 1);
    assert_eq!(stats.heatmap.iter().map(|d| d.count).sum::<u64>(), 3);
    assert_eq!(stats.reading_completion, Some(1.));
    assert_eq!(stats.learned_total, 1);
    assert_eq!(stats.experiment.exposed.sample, 1);
    assert_eq!(stats.experiment.exposed.again, 1);
    assert_eq!(stats.experiment.control.unreviewed, 1);
    assert_eq!(stats.experiment.difference_percentage_points, None);
    assert!(
        s.experiment_report("p", 100000, "legacy")
            .unwrap()
            .exposed
            .again_rate
            .is_none()
    );
}
