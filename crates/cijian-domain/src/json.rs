//! Stable JSON hashing input: object keys use Rust string order; array order is retained.
use serde_json::Value;
pub fn canonical_json(value: &Value) -> String {
    fn append(value: &Value, output: &mut String) {
        match value {
            Value::Object(object) => {
                output.push('{');
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort_unstable();
                for (index, key) in keys.into_iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    output
                        .push_str(&serde_json::to_string(key).expect("JSON string serialization"));
                    output.push(':');
                    append(&object[key], output);
                }
                output.push('}');
            }
            Value::Array(values) => {
                output.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    append(value, output);
                }
                output.push(']');
            }
            _ => output.push_str(&value.to_string()),
        }
    }
    let mut output = String::new();
    append(value, &mut output);
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn object_insertion_order_does_not_change_digest_input() {
        let a: Value = serde_json::from_str(r#"{"z":{"b":2,"a":1},"a":[3,2]}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"a":[3,2],"z":{"a":1,"b":2}}"#).unwrap();
        assert_eq!(canonical_json(&a), canonical_json(&b));
        assert_eq!(canonical_json(&a), r#"{"a":[3,2],"z":{"a":1,"b":2}}"#);
    }
}
