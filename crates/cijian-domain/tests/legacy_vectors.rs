use chrono::DateTime;
use cijian_domain::srs::{INITIAL_EASE_FACTOR, SrsState, State, preview};
use serde_json::Value;
fn ms(value: &Value) -> i64 {
    DateTime::parse_from_rfc3339(value.as_str().unwrap())
        .unwrap()
        .timestamp_millis()
}
#[test]
fn legacy_memory_vectors_match() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../spikes/r00/fsrs-vectors.json")).unwrap();
    let now = ms(&fixture["now"]);
    for case in fixture["cases"].as_array().unwrap() {
        let c = &case["card"];
        let state = |v: &Value| {
            [
                State::New,
                State::Learning,
                State::Review,
                State::Relearning,
            ][v.as_u64().unwrap() as usize]
        };
        let input = SrsState {
            state: state(&c["state"]),
            due: ms(&c["due"]),
            stability: c["stability"].as_f64().unwrap(),
            difficulty: c["difficulty"].as_f64().unwrap(),
            reps: c["reps"].as_u64().unwrap() as u32,
            lapses: c["lapses"].as_u64().unwrap() as u32,
            last_review: c.get("last_review").map(ms),
            elapsed_days: c["elapsed_days"].as_u64().unwrap() as u32,
            scheduled_days: c["scheduled_days"].as_u64().unwrap() as u32,
            learning_steps: c["learning_steps"].as_u64().unwrap() as u32,
            ease_factor: INITIAL_EASE_FACTOR,
        };
        // This fixture is a ts-fsrs memory-state vector. Due times, steps and
        // review intervals are now governed by Anki's scheduler and are
        // covered by the srs unit tests instead.
        for (index, actual) in preview(&input, now, 0.9).unwrap().iter().enumerate() {
            let expected = &case["ratings"][index]["card"];
            let context = format!("{} rating {}", case["name"], index + 1);
            assert!(
                (actual.stability - expected["stability"].as_f64().unwrap()).abs() < 0.0001,
                "{context}"
            );
            assert!(
                (actual.difficulty - expected["difficulty"].as_f64().unwrap()).abs() < 0.0001,
                "{context}"
            );
            assert_eq!(
                u64::from(actual.reps),
                expected["reps"].as_u64().unwrap(),
                "{context} reps"
            );
            assert_eq!(
                actual.last_review,
                Some(ms(&expected["last_review"])),
                "{context}"
            );
        }
    }
}
