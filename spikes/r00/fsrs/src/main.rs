use fsrs::{FSRS, MemoryState};
use serde_json::{Value, json};

fn compare() -> Value {
    let vectors: Value = serde_json::from_str(include_str!("../../fsrs-vectors.json")).unwrap();
    let parameters: Vec<f32> = vectors["parameters"]["w"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap() as f32)
        .collect();
    let scheduler = FSRS::new(&parameters).unwrap();
    let mut results = Vec::new();
    for case in vectors["cases"].as_array().unwrap() {
        let card = &case["card"];
        let memory = (card["state"] != 0).then(|| MemoryState {
            stability: card["stability"].as_f64().unwrap() as f32,
            difficulty: card["difficulty"].as_f64().unwrap() as f32,
        });
        let elapsed = card["elapsed_days"].as_u64().unwrap() as u32;
        let next = scheduler
            .next_states(memory, 0.9, elapsed)
            .unwrap();
        for (i, state) in [next.again, next.hard, next.good, next.easy]
            .iter()
            .enumerate()
        {
            let expected = &case["ratings"][i]["card"];
            let stability_delta =
                (f64::from(state.memory.stability) - expected["stability"].as_f64().unwrap()).abs();
            let difficulty_delta = (f64::from(state.memory.difficulty)
                - expected["difficulty"].as_f64().unwrap())
            .abs();
            results.push(json!({
                "case": case["name"], "rating": i + 1,
                "rustMemory": {"stability": state.memory.stability, "difficulty": state.memory.difficulty},
                "stabilityDelta": stability_delta, "difficultyDelta": difficulty_delta,
                "memoryMatches": stability_delta < 0.0001 && difficulty_delta < 0.0001,
                "rustRawIntervalDays": state.interval,
                "legacyScheduledDays": expected["scheduled_days"],
                "legacyDue": expected["due"],
            }));
        }
    }
    json!({"candidate": "fsrs 6.6.2", "results": results,
        "limitation": "Memory transition comparison only. Learning steps, interval constraints, due time, repetitions and lapses require the application scheduler; this is not full scheduler equivalence."})
}

fn main() {
    println!("{}", serde_json::to_string_pretty(&compare()).unwrap());
}

#[cfg(test)]
mod tests {
    #[test]
    fn legacy_memory_transitions_match_all_four_ratings() {
        let report = super::compare();
        let results = report["results"].as_array().unwrap();
        assert_eq!(results.len(), 64);
        for result in results {
            assert_eq!(result["memoryMatches"], true, "{result}");
        }
    }
}
