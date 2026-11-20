use beacon_core::Decision;
use serde_json::{Map, Value, json};
use std::path::PathBuf;

pub const USAGE: &str = "\
Usage:
  beacon daemon
  beacon stop
  beacon status [--watch] [--json]
  beacon decide <id> allow
  beacon decide <id> deny [--message <text>] [--stop]
  beacon decide <id> answer \"<question>=<label>\" ...
  beacon decide <id> always \"<Tool(specifier)>\" [--scope session|local|project|user]
  beacon ack <session-id-or-unique-prefix>
  beacon jump <session-id-or-unique-prefix>
  beacon where <session-id-or-unique-prefix>
  beacon snooze <session-id-or-unique-prefix|all> <minutes>
  beacon unsnooze <session-id-or-unique-prefix|all>
  beacon usage [--refresh]
  beacon install [--settings <path>] [--yes]
  beacon uninstall [--settings <path>] [--yes]
  beacon doctor [--settings <path>]
  beacon help
";

#[derive(Debug, PartialEq)]
pub enum Command {
    Help,
    Daemon,
    Stop,
    Status {
        watch: bool,
        json: bool,
    },
    Decide {
        id: u64,
        decision: Decision,
    },
    Ack {
        session: String,
    },
    Jump {
        session: String,
    },
    Where {
        session: String,
    },
    Snooze {
        session: Option<String>,
        minutes: u64,
    },
    Unsnooze {
        session: Option<String>,
    },
    Usage {
        refresh: bool,
    },
    Install {
        settings: Option<PathBuf>,
        yes: bool,
    },
    Uninstall {
        settings: Option<PathBuf>,
        yes: bool,
    },
    Doctor {
        settings: Option<PathBuf>,
    },
}

#[derive(Default)]
struct Options {
    switches: Vec<String>,
    values: Vec<(String, String)>,
}

impl Options {
    fn parse(args: &[String], switches: &[&str], valued: &[&str]) -> Result<Options, String> {
        let mut options = Options::default();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if switches.contains(&arg.as_str()) {
                options.switches.push(arg.clone());
            } else if valued.contains(&arg.as_str()) {
                let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
                options.values.push((arg.clone(), value.clone()));
            } else {
                return Err(format!("unexpected argument: {arg}"));
            }
        }
        Ok(options)
    }

    fn has(&self, switch: &str) -> bool {
        self.switches.iter().any(|given| given == switch)
    }

    fn value(&self, name: &str) -> Option<String> {
        self.values
            .iter()
            .rev()
            .find(|(given, _)| given == name)
            .map(|(_, value)| value.clone())
    }
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let Some((command, rest)) = args.split_first() else {
        return Ok(Command::Help);
    };
    match (command.as_str(), rest) {
        ("help" | "--help" | "-h", []) => Ok(Command::Help),
        ("daemon", []) => Ok(Command::Daemon),
        ("stop", []) => Ok(Command::Stop),
        ("status", _) => {
            let options = Options::parse(rest, &["--watch", "--json"], &[])?;
            Ok(Command::Status {
                watch: options.has("--watch"),
                json: options.has("--json"),
            })
        }
        ("decide", _) => parse_decide(rest),
        ("ack", [session]) => Ok(Command::Ack {
            session: session.clone(),
        }),
        ("jump", [session]) => Ok(Command::Jump {
            session: session.clone(),
        }),
        ("where", [session]) => Ok(Command::Where {
            session: session.clone(),
        }),
        ("snooze", [session, minutes]) => Ok(Command::Snooze {
            session: session_or_all(session),
            minutes: minutes
                .parse::<u64>()
                .ok()
                .filter(|minutes| *minutes > 0)
                .ok_or_else(|| format!("not a number of minutes: {minutes}"))?,
        }),
        ("unsnooze", [session]) => Ok(Command::Unsnooze {
            session: session_or_all(session),
        }),
        ("usage", _) => Ok(Command::Usage {
            refresh: Options::parse(rest, &["--refresh"], &[])?.has("--refresh"),
        }),
        ("install" | "uninstall", _) => {
            let options = Options::parse(rest, &["--yes"], &["--settings"])?;
            let settings = options.value("--settings").map(PathBuf::from);
            let yes = options.has("--yes");
            Ok(if command == "install" {
                Command::Install { settings, yes }
            } else {
                Command::Uninstall { settings, yes }
            })
        }
        ("doctor", _) => Ok(Command::Doctor {
            settings: Options::parse(rest, &[], &["--settings"])?
                .value("--settings")
                .map(PathBuf::from),
        }),
        _ => Err(format!("unknown command: {}", args.join(" "))),
    }
}

fn session_or_all(session: &str) -> Option<String> {
    (session != "all").then(|| session.to_string())
}

fn parse_decide(args: &[String]) -> Result<Command, String> {
    let [id, verb, rest @ ..] = args else {
        return Err("decide needs a decision id and allow, always, deny or answer".to_string());
    };
    let id = id
        .parse::<u64>()
        .map_err(|_| format!("not a decision id: {id}"))?;
    let decision = match (verb.as_str(), rest) {
        ("allow", []) => Decision::Allow,
        ("deny", _) => {
            let options = Options::parse(rest, &["--stop"], &["--message"])?;
            let message = options.value("--message");
            if options.has("--stop") {
                Decision::DenyAndStop { message }
            } else {
                Decision::Deny { message }
            }
        }
        ("always", [rule, options @ ..]) => {
            let scope = Options::parse(options, &[], &["--scope"])?.value("--scope");
            Decision::AllowAlways {
                permissions: vec![always_rule(rule, scope.as_deref().unwrap_or("session"))?],
            }
        }
        ("answer", [_, ..]) => Decision::Answer {
            answers: rest
                .iter()
                .map(|pair| answer(pair))
                .collect::<Result<Map<String, Value>, String>>()?,
        },
        _ => return Err(format!("unknown decision: {}", args[1..].join(" "))),
    };
    Ok(Command::Decide { id, decision })
}

fn always_rule(rule: &str, scope: &str) -> Result<Value, String> {
    let destination = match scope {
        "session" => "session",
        "local" => "localSettings",
        "project" => "projectSettings",
        "user" => "userSettings",
        _ => {
            return Err(format!(
                "unknown scope {scope}: expected session, local, project or user"
            ));
        }
    };
    let parsed = match rule.split_once('(') {
        None => Some((rule, None)),
        Some((tool, rest)) => rest
            .strip_suffix(')')
            .filter(|content| !content.is_empty())
            .map(|content| (tool, Some(content))),
    };
    let Some((tool, content)) = parsed.filter(|(tool, _)| {
        !tool.is_empty()
            && !tool.contains(|character: char| character.is_whitespace() || character == ')')
    }) else {
        return Err(format!(
            "expected a rule like Bash or Bash(npm test *), got: {rule}"
        ));
    };
    let mut entry = json!({"toolName": tool});
    if let Some(content) = content {
        entry["ruleContent"] = json!(content);
    }
    Ok(
        json!({"type": "addRules", "rules": [entry], "behavior": "allow", "destination": destination}),
    )
}

fn answer(pair: &str) -> Result<(String, Value), String> {
    match pair.rsplit_once('=') {
        Some((question, label)) if !question.is_empty() && !label.is_empty() => {
            Ok((question.to_string(), Value::String(label.to_string())))
        }
        _ => Err(format!("expected \"<question>=<label>\", got: {pair}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parsed(line: &[&str]) -> Result<Command, String> {
        parse(&line.iter().map(|arg| arg.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn simple_commands_parse() {
        assert_eq!(parsed(&[]), Ok(Command::Help));
        assert_eq!(parsed(&["help"]), Ok(Command::Help));
        assert_eq!(parsed(&["daemon"]), Ok(Command::Daemon));
        assert_eq!(parsed(&["stop"]), Ok(Command::Stop));
        assert_eq!(
            parsed(&["status"]),
            Ok(Command::Status {
                watch: false,
                json: false
            })
        );
        assert_eq!(
            parsed(&["status", "--json", "--watch"]),
            Ok(Command::Status {
                watch: true,
                json: true
            })
        );
        assert_eq!(
            parsed(&["ack", "4c2c"]),
            Ok(Command::Ack {
                session: "4c2c".to_string()
            })
        );
    }

    #[test]
    fn snooze_and_usage_commands_parse() {
        assert_eq!(
            parsed(&["snooze", "4c2c", "30"]),
            Ok(Command::Snooze {
                session: Some("4c2c".to_string()),
                minutes: 30
            })
        );
        assert_eq!(
            parsed(&["snooze", "all", "5"]),
            Ok(Command::Snooze {
                session: None,
                minutes: 5
            })
        );
        assert_eq!(
            parsed(&["unsnooze", "4c2c"]),
            Ok(Command::Unsnooze {
                session: Some("4c2c".to_string())
            })
        );
        assert_eq!(
            parsed(&["unsnooze", "all"]),
            Ok(Command::Unsnooze { session: None })
        );
        assert_eq!(parsed(&["usage"]), Ok(Command::Usage { refresh: false }));
        assert_eq!(
            parsed(&["usage", "--refresh"]),
            Ok(Command::Usage { refresh: true })
        );
        for line in [
            &["snooze"][..],
            &["snooze", "all"],
            &["snooze", "all", "0"],
            &["snooze", "all", "-5"],
            &["snooze", "all", "soon"],
            &["snooze", "a", "5", "extra"],
            &["unsnooze"],
            &["unsnooze", "a", "b"],
            &["usage", "--json"],
        ] {
            assert!(parsed(line).is_err(), "{line:?}");
        }
    }

    #[test]
    fn decisions_parse() {
        assert_eq!(
            parsed(&["decide", "7", "allow"]),
            Ok(Command::Decide {
                id: 7,
                decision: Decision::Allow
            })
        );
        assert_eq!(
            parsed(&["decide", "7", "deny"]),
            Ok(Command::Decide {
                id: 7,
                decision: Decision::Deny { message: None }
            })
        );
        assert_eq!(
            parsed(&["decide", "7", "deny", "--stop", "--message", "not now"]),
            Ok(Command::Decide {
                id: 7,
                decision: Decision::DenyAndStop {
                    message: Some("not now".to_string())
                }
            })
        );
        let Ok(Command::Decide {
            decision: Decision::Answer { answers },
            ..
        }) = parsed(&["decide", "2", "answer", "Which db?=Postgres", "a=b=c"])
        else {
            panic!("expected answers");
        };
        assert_eq!(
            Value::Object(answers),
            json!({"Which db?": "Postgres", "a=b": "c"})
        );
    }

    fn always(line: &[&str]) -> Value {
        let Ok(Command::Decide {
            id: 4,
            decision: Decision::AllowAlways { permissions },
        }) = parsed(line)
        else {
            panic!("expected an always decision for {line:?}");
        };
        Value::Array(permissions)
    }

    #[test]
    fn always_builds_one_add_rules_entry() {
        assert_eq!(
            serde_json::to_string(&always(&["decide", "4", "always", "Bash(node -e *)"])).unwrap(),
            r#"[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"node -e *"}],"behavior":"allow","destination":"session"}]"#
        );
        assert_eq!(
            always(&["decide", "4", "always", "WebFetch", "--scope", "user"]),
            json!([{"type": "addRules", "rules": [{"toolName": "WebFetch"}], "behavior": "allow", "destination": "userSettings"}])
        );
        assert_eq!(
            always(&[
                "decide",
                "4",
                "always",
                "Bash(node -e \"console.log('x')\")",
                "--scope",
                "local"
            ]),
            json!([{"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "node -e \"console.log('x')\""}], "behavior": "allow", "destination": "localSettings"}])
        );
        assert_eq!(
            always(&["decide", "4", "always", "mcp__github", "--scope", "project"])[0]["destination"],
            "projectSettings"
        );
        for line in [
            &["decide", "4", "always"][..],
            &["decide", "4", "always", "Bash("],
            &["decide", "4", "always", "Bash()"],
            &["decide", "4", "always", "(npm test)"],
            &["decide", "4", "always", "Bash(x) extra"],
            &["decide", "4", "always", "Bash(x)y"],
            &["decide", "4", "always", "Read File"],
            &["decide", "4", "always", "Bash", "--scope", "everywhere"],
            &["decide", "4", "always", "Bash", "--scope"],
            &["decide", "4", "always", "Bash", "--global"],
        ] {
            assert!(parsed(line).is_err(), "{line:?}");
        }
    }

    #[test]
    fn installer_commands_parse() {
        assert_eq!(
            parsed(&["install"]),
            Ok(Command::Install {
                settings: None,
                yes: false
            })
        );
        assert_eq!(
            parsed(&["uninstall", "--yes", "--settings", r"C:\t\s.json"]),
            Ok(Command::Uninstall {
                settings: Some(PathBuf::from(r"C:\t\s.json")),
                yes: true
            })
        );
        assert_eq!(
            parsed(&["doctor", "--settings", "s.json"]),
            Ok(Command::Doctor {
                settings: Some(PathBuf::from("s.json"))
            })
        );
    }

    #[test]
    fn jump_and_where_parse() {
        assert_eq!(
            parsed(&["jump", "4c2c"]),
            Ok(Command::Jump {
                session: "4c2c".to_string()
            })
        );
        assert_eq!(
            parsed(&["where", "4c2c"]),
            Ok(Command::Where {
                session: "4c2c".to_string()
            })
        );
        for line in [
            &["jump"][..],
            &["jump", "a", "b"],
            &["where"],
            &["where", "a", "--json"],
        ] {
            assert!(parsed(line).is_err(), "{line:?}");
        }
        assert!(USAGE.contains("beacon jump <session-id-or-unique-prefix>"));
        assert!(USAGE.contains("beacon where <session-id-or-unique-prefix>"));
    }

    #[test]
    fn bad_input_is_refused() {
        for line in [
            &["frobnicate"][..],
            &["daemon", "now"],
            &["stop", "--force"],
            &["status", "--colour"],
            &["decide"],
            &["decide", "x", "allow"],
            &["decide", "1", "allow", "extra"],
            &["decide", "1", "maybe"],
            &["decide", "1", "answer"],
            &["decide", "1", "answer", "no-equals"],
            &["decide", "1", "deny", "--message"],
            &["ack"],
            &["ack", "a", "b"],
            &["install", "--force"],
            &["doctor", "--yes"],
            &["help", "me"],
        ] {
            assert!(parsed(line).is_err(), "{line:?}");
        }
    }
}
