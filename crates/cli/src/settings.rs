use beacon_protocol::{DECISION_TIMEOUT_SECS, STOP_WAITER_EVENT};
use serde::{Deserialize, Serialize};
use serde_json::ser::{PrettyFormatter, Serializer};
use serde_json::{Map, Value, json};

pub const HELPER_FILE_NAME: &str = "beacon-hook.exe";
const BOM: char = '\u{feff}';
const STOP_WAITER_TIMEOUT_SECS: u64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    Async,
    Blocking(Option<u64>),
    Rewake(u64),
}

pub const HANDLERS: [(&str, &str, Shape); 23] = [
    ("SessionStart", "SessionStart", Shape::Async),
    ("UserPromptSubmit", "UserPromptSubmit", Shape::Async),
    ("PreToolUse", "PreToolUse", Shape::Async),
    ("PostToolUse", "PostToolUse", Shape::Async),
    ("PostToolUseFailure", "PostToolUseFailure", Shape::Async),
    ("PostToolBatch", "PostToolBatch", Shape::Async),
    ("PermissionDenied", "PermissionDenied", Shape::Async),
    ("Notification", "Notification", Shape::Async),
    ("SubagentStart", "SubagentStart", Shape::Async),
    ("SubagentStop", "SubagentStop", Shape::Async),
    ("TaskCreated", "TaskCreated", Shape::Async),
    ("TaskCompleted", "TaskCompleted", Shape::Async),
    ("PreCompact", "PreCompact", Shape::Async),
    ("PostCompact", "PostCompact", Shape::Async),
    ("CwdChanged", "CwdChanged", Shape::Async),
    ("PostModelSwitch", "PostModelSwitch", Shape::Async),
    ("ElicitationResult", "ElicitationResult", Shape::Async),
    (
        "PermissionRequest",
        "PermissionRequest",
        Shape::Blocking(Some(DECISION_TIMEOUT_SECS)),
    ),
    (
        "Elicitation",
        "Elicitation",
        Shape::Blocking(Some(DECISION_TIMEOUT_SECS)),
    ),
    ("Stop", "Stop", Shape::Blocking(Some(10))),
    (
        "Stop",
        STOP_WAITER_EVENT,
        Shape::Rewake(STOP_WAITER_TIMEOUT_SECS),
    ),
    ("SessionEnd", "SessionEnd", Shape::Blocking(None)),
    ("StopFailure", "StopFailure", Shape::Blocking(None)),
];

#[derive(Debug, Clone, PartialEq)]
pub struct TextFormat {
    indent: String,
    crlf: bool,
    bom: bool,
    trailing_newline: bool,
}

impl Default for TextFormat {
    fn default() -> TextFormat {
        TextFormat {
            indent: "  ".to_string(),
            crlf: false,
            bom: false,
            trailing_newline: true,
        }
    }
}

impl TextFormat {
    pub fn write(&self, settings: &Map<String, Value>) -> Vec<u8> {
        let mut bytes = Vec::new();
        settings
            .serialize(&mut Serializer::with_formatter(
                &mut bytes,
                PrettyFormatter::with_indent(self.indent.as_bytes()),
            ))
            .expect("serializing JSON into memory cannot fail");
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if self.trailing_newline {
            text.push('\n');
        }
        if self.crlf {
            text = text.replace('\n', "\r\n");
        }
        if self.bom {
            text.insert(0, BOM);
        }
        text.into_bytes()
    }
}

pub fn parse(bytes: &[u8]) -> Result<(Map<String, Value>, TextFormat), String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("not UTF-8 text: {error}"))?;
    let body = text.strip_prefix(BOM).unwrap_or(text);
    let settings = match serde_json::from_str::<Value>(body) {
        Ok(Value::Object(settings)) => settings,
        Ok(_) => return Err("the top level is not a JSON object".to_string()),
        Err(error) => return Err(format!("not valid JSON: {error}")),
    };
    let indent = body
        .lines()
        .skip(1)
        .map(|line| &line[..line.len() - line.trim_start_matches([' ', '\t']).len()])
        .find(|indent| !indent.is_empty())
        .unwrap_or("  ");
    Ok((
        settings,
        TextFormat {
            indent: indent.to_string(),
            crlf: body.contains("\r\n"),
            bom: body.len() != text.len(),
            trailing_newline: body.ends_with('\n'),
        },
    ))
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Created {
    pub hooks: bool,
    pub events: Vec<String>,
}

impl Created {
    pub fn includes(&self, container: Option<&str>) -> bool {
        match container {
            None => self.hooks,
            Some(event) => self.events.iter().any(|created| created == event),
        }
    }
}

pub fn beacon_handler(argument: &str, shape: Shape, helper: &str) -> Value {
    let mut handler = json!({"type": "command", "command": helper, "args": [argument]});
    match shape {
        Shape::Async => handler["async"] = json!(true),
        Shape::Blocking(Some(timeout)) => handler["timeout"] = json!(timeout),
        Shape::Blocking(None) => {}
        Shape::Rewake(timeout) => {
            handler["asyncRewake"] = json!(true);
            handler["timeout"] = json!(timeout);
        }
    }
    handler
}

fn handler_argument(handler: &Value) -> Option<&str> {
    handler.get("args")?.get(0)?.as_str()
}

pub fn is_beacon_handler(handler: &Value) -> bool {
    handler
        .get("command")
        .and_then(Value::as_str)
        .and_then(|command| command.rsplit(['\\', '/']).next())
        .is_some_and(|name| name.eq_ignore_ascii_case(HELPER_FILE_NAME))
}

fn beacon_handlers(groups: &Value) -> impl Iterator<Item = &Value> {
    groups
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|group| group.get("hooks")?.as_array())
        .flatten()
        .filter(|handler| is_beacon_handler(handler))
}

pub fn has_beacon(settings: &Map<String, Value>) -> bool {
    settings
        .get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|hooks| hooks.values())
        .any(|groups| beacon_handlers(groups).next().is_some())
}

pub fn strip_beacon(settings: &mut Map<String, Value>, may_drop: impl Fn(Option<&str>) -> bool) {
    let Some(Value::Object(hooks)) = settings.get_mut("hooks") else {
        return;
    };
    let hooks_had_events = !hooks.is_empty();
    let mut emptied = Vec::new();
    for (event, groups) in hooks.iter_mut() {
        let Value::Array(groups) = groups else {
            continue;
        };
        let had_groups = !groups.is_empty();
        groups.retain_mut(|group| {
            let Some(Value::Array(handlers)) = group.get_mut("hooks") else {
                return true;
            };
            let had_handlers = !handlers.is_empty();
            handlers.retain(|handler| !is_beacon_handler(handler));
            !(had_handlers && handlers.is_empty())
        });
        if had_groups && groups.is_empty() && may_drop(Some(event)) {
            emptied.push(event.clone());
        }
    }
    for event in &emptied {
        hooks.shift_remove(event);
    }
    if hooks_had_events && hooks.is_empty() && may_drop(None) {
        settings.shift_remove("hooks");
    }
}

pub fn add_beacon(settings: &mut Map<String, Value>, helper: &str) -> Result<Created, String> {
    let mut created = Created {
        hooks: !settings.contains_key("hooks"),
        events: Vec::new(),
    };
    let Value::Object(hooks) = settings
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
    else {
        return Err("\"hooks\" is not a JSON object".to_string());
    };
    for (event, argument, shape) in HANDLERS {
        if !hooks.contains_key(event) {
            created.events.push(event.to_string());
        }
        let Value::Array(groups) = hooks
            .entry(event)
            .or_insert_with(|| Value::Array(Vec::new()))
        else {
            return Err(format!("\"hooks\".\"{event}\" is not a JSON array"));
        };
        groups.push(json!({"hooks": [beacon_handler(argument, shape, helper)]}));
    }
    Ok(created)
}

pub fn registration_problems(settings: &Map<String, Value>, helper: &str) -> Vec<String> {
    let hooks = settings.get("hooks").and_then(Value::as_object);
    let mut problems = Vec::new();
    for (event, argument, shape) in HANDLERS {
        let found: Vec<&Value> = hooks
            .and_then(|hooks| hooks.get(event))
            .into_iter()
            .flat_map(beacon_handlers)
            .filter(|handler| handler_argument(handler) == Some(argument))
            .collect();
        match found.as_slice() {
            [] => problems.push(format!("{argument} is not registered")),
            [handler] if **handler == beacon_handler(argument, shape, helper) => {}
            [_] => problems.push(format!("{argument} has the wrong helper path or shape")),
            several => problems.push(format!("{argument} is registered {} times", several.len())),
        }
    }
    for (event, groups) in hooks.into_iter().flatten() {
        let arguments: Vec<&str> = HANDLERS
            .iter()
            .filter(|(beacon_event, ..)| beacon_event == event)
            .map(|(_, argument, _)| *argument)
            .collect();
        let stray = beacon_handlers(groups).any(|handler| {
            !handler_argument(handler).is_some_and(|argument| arguments.contains(&argument))
        });
        match (stray, arguments.is_empty()) {
            (true, true) => problems.push(format!(
                "{event} has a Beacon handler but is not a Beacon event"
            )),
            (true, false) => problems.push(format!(
                "{event} has a Beacon handler that Beacon does not register"
            )),
            (false, _) => {}
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_is_detected_from_the_text() {
        let (_, format) = parse(b"\xef\xbb\xbf{\r\n\t\"a\": 1\r\n}").unwrap();
        assert_eq!(
            format,
            TextFormat {
                indent: "\t".to_string(),
                crlf: true,
                bom: true,
                trailing_newline: false
            }
        );
        let (_, format) = parse(b"{}").unwrap();
        assert_eq!(
            format,
            TextFormat {
                trailing_newline: false,
                ..TextFormat::default()
            }
        );
        let (_, format) = parse(b"{\n    \"a\": {\n        \"b\": 1\n    }\n}\n").unwrap();
        assert_eq!(format.indent, "    ");
    }

    #[test]
    fn non_objects_and_invalid_json_are_refused() {
        assert_eq!(
            parse(b"[1, 2]\n").unwrap_err(),
            "the top level is not a JSON object"
        );
        assert!(
            parse(b"{\"a\": 1,}")
                .unwrap_err()
                .starts_with("not valid JSON")
        );
        assert!(parse(b"").unwrap_err().starts_with("not valid JSON"));
        assert!(
            parse(b"{\"a\": \"\xff\"}")
                .unwrap_err()
                .starts_with("not UTF-8")
        );
    }

    #[test]
    fn beacon_handlers_are_recognised_by_file_name() {
        for command in [
            r"C:\Users\x\AppData\Local\Beacon\bin\beacon-hook.exe",
            "D:/elsewhere/BEACON-HOOK.EXE",
            "beacon-hook.exe",
        ] {
            assert!(is_beacon_handler(&json!({ "command": command })));
        }
        for handler in [
            json!({"command": r"C:\tools\beacon-hook.exe.bak"}),
            json!({"command": "node beacon-hook.js"}),
            json!({"type": "command"}),
        ] {
            assert!(!is_beacon_handler(&handler));
        }
    }

    #[test]
    fn handlers_have_the_registered_shapes() {
        assert_eq!(
            beacon_handler("PreToolUse", Shape::Async, r"C:\b\beacon-hook.exe").to_string(),
            r#"{"type":"command","command":"C:\\b\\beacon-hook.exe","args":["PreToolUse"],"async":true}"#
        );
        assert_eq!(
            beacon_handler("Stop", Shape::Blocking(Some(10)), "h").to_string(),
            r#"{"type":"command","command":"h","args":["Stop"],"timeout":10}"#
        );
        assert_eq!(
            beacon_handler("SessionEnd", Shape::Blocking(None), "h").to_string(),
            r#"{"type":"command","command":"h","args":["SessionEnd"]}"#
        );
        assert_eq!(
            beacon_handler("StopWaiter", Shape::Rewake(86_400), "h").to_string(),
            r#"{"type":"command","command":"h","args":["StopWaiter"],"asyncRewake":true,"timeout":86400}"#
        );
        let registered: Vec<&str> = HANDLERS.iter().map(|(event, ..)| *event).collect();
        for never in [
            "WorktreeCreate",
            "WorktreeRemove",
            "PreModelSwitch",
            "MessageDisplay",
            "FileChanged",
        ] {
            assert!(!registered.contains(&never));
        }
    }

    const HELPER: &str = r"C:\b\beacon-hook.exe";

    #[test]
    fn registration_problems_name_each_fault() {
        let mut settings = Map::new();
        add_beacon(&mut settings, HELPER).unwrap();
        assert!(registration_problems(&settings, HELPER).is_empty());
        let hooks = settings["hooks"].as_object_mut().unwrap();
        hooks.shift_remove("Stop");
        hooks["SessionEnd"][0]["hooks"][0]["timeout"] = json!(5);
        hooks["PreToolUse"]
            .as_array_mut()
            .unwrap()
            .push(json!({"hooks": [beacon_handler("PreToolUse", Shape::Async, HELPER)]}));
        hooks.insert(
            "WorktreeCreate".to_string(),
            json!([{"hooks": [{"type": "command", "command": "beacon-hook.exe"}]}]),
        );
        assert_eq!(
            registration_problems(&settings, HELPER),
            [
                "PreToolUse is registered 2 times",
                "Stop is not registered",
                "StopWaiter is not registered",
                "SessionEnd has the wrong helper path or shape",
                "WorktreeCreate has a Beacon handler but is not a Beacon event"
            ]
        );
        let mut settings = Map::new();
        add_beacon(&mut settings, HELPER).unwrap();
        let stop = settings["hooks"]["Stop"].as_array_mut().unwrap();
        assert_eq!(stop.len(), 2);
        stop[1]["hooks"][0]["args"] = json!(["StopWaiting"]);
        assert_eq!(
            registration_problems(&settings, HELPER),
            [
                "StopWaiter is not registered",
                "Stop has a Beacon handler that Beacon does not register"
            ]
        );
        assert_eq!(
            registration_problems(&settings, r"C:\other\beacon-hook.exe")[0],
            "SessionStart has the wrong helper path or shape"
        );
    }
}
