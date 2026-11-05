use chrono::DateTime;
use cijian_domain::srs::{SrsState, State, preview};
use serde_json::Value;
fn ms(value: &Value) -> i64 {
    DateTime::parse_from_rfc3339(value.as_str().unwrap())
        .unwrap()
        .timestamp_millis()
}
#[test]
fn legacy_memory_vectors_and_review_schedule_match() {
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
        };
        // This fixture is a ts-fsrs memory-state vector. New/Learning due
        // times and steps are intentionally governed by Anki's learning-step
        // policy and are covered by the srs unit tests instead.
        let anki_learning = matches!(input.state, State::New | State::Learning);
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
            if anki_learning {
                continue;
            }
            assert_eq!(actual.due, ms(&expected["due"]), "{context}");
            assert_eq!(actual.state, state(&expected["state"]), "{context}");
            for (name, value) in [
                ("lapses", actual.lapses),
                ("elapsed_days", actual.elapsed_days),
                ("scheduled_days", actual.scheduled_days),
                ("learning_steps", actual.learning_steps),
            ] {
                assert_eq!(
                    u64::from(value),
                    expected[name].as_u64().unwrap(),
                    "{context} {name}"
                );
            }
        }
    }
}
