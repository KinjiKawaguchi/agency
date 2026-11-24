//! 決まった順の JSON。キーをバイト順に並べ、空白を入れず、値が null の項目を書かない。
//! 署名する文字列は、作る台と確かめる台で 1 バイトも違ってはいけないので、ここで 1 つに決める。
use serde_json::Value;

pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write(value, &mut out);
    out
}

pub fn canonical_of<T: serde::Serialize>(value: &T) -> serde_json::Result<String> {
    Ok(canonical(&serde_json::to_value(value)?))
}

fn write(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().filter(|(_, one)| !one.is_null()).collect();
            entries.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
            out.push('{');
            for (i, (key, one)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write(one, out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, one) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(one, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorts_keys_drops_nulls_and_keeps_array_order() {
        let value = json!({ "b": 1, "a": { "d": [3, 1], "c": null }, "日本": "語" });
        assert_eq!(canonical(&value), r#"{"a":{"d":[3,1]},"b":1,"日本":"語"}"#);
    }

    #[test]
    fn same_text_regardless_of_input_order() {
        assert_eq!(canonical(&json!({ "x": 1, "y": 2 })), canonical(&json!({ "y": 2, "x": 1 })));
    }
}
