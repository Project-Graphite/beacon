use crate::redact::{redact, redact_json};
use serde_json::Value;

const INPUT_CHARS: usize = 80;

pub fn input_preview(tool_name: &str, tool_input: &Value) -> String {
    let field = |name: &str| tool_input.get(name).and_then(Value::as_str);
    let subject = match tool_name {
        "Bash" | "PowerShell" => field("command").map(str::to_string),
        "Edit" | "Write" | "Read" => field("file_path").map(str::to_string),
        "WebFetch" => field("url").map(str::to_string),
        tool => tool
            .strip_prefix("mcp__")
            .map(|server_and_tool| server_and_tool.replacen("__", "/", 1)),
    };
    one_line(
        &subject.map_or_else(
            || redact_json("", tool_input).to_string(),
            |subject| redact(&subject),
        ),
        INPUT_CHARS,
    )
}

pub fn one_line(text: &str, limit: usize) -> String {
    let flat: Vec<char> = text
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    if flat.len() <= limit {
        return flat.into_iter().collect();
    }
    flat[..limit - 3].iter().chain(&['.'; 3]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn previews_pick_the_subject_of_each_tool_and_redact_it() {
        let cases = [
            (
                "Bash",
                json!({"command": "curl -H \"Authorization: Bearer abc\"\nhttps://x.test", "description": "fetch"}),
                "curl -H \"Authorization: ***\" https://x.test",
            ),
            (
                "PowerShell",
                json!({"command": "API_KEY=abc123 npm run deploy"}),
                "API_KEY=*** npm run deploy",
            ),
            (
                "mcp__github__get_issue",
                json!({"number": 1}),
                "github/get_issue",
            ),
            ("Edit", json!({"file_path": r"C:\x\a.rs"}), r"C:\x\a.rs"),
            (
                "WebFetch",
                json!({"url": "https://example.test/?api_key=k"}),
                "https://example.test/?api_key=***",
            ),
            (
                "AskUserQuestion",
                json!({"questions": [{"question": "Which?"}]}),
                r#"{"questions":[{"question":"Which?"}]}"#,
            ),
            (
                "Task",
                json!({"config": "{\"api_key\": \"abc123secret\"}", "note": "password: \"hunter2\""}),
                r#"{"config":"{\"api_key\": ***}","note":"password: ***"}"#,
            ),
        ];
        for (tool_name, tool_input, expected) in cases {
            assert_eq!(
                input_preview(tool_name, &tool_input),
                expected,
                "{tool_name}"
            );
        }
    }

    #[test]
    fn long_inputs_are_cut_to_one_line() {
        let long = format!("echo {}", "x".repeat(200));
        let preview = input_preview("PowerShell", &json!({ "command": long }));
        assert_eq!(preview.chars().count(), 80);
        assert!(preview.ends_with("xx..."));
    }
}
