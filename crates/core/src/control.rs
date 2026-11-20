use crate::account::AccountUsage;
use crate::platform;
use crate::session::Session;
use crate::state::PermissionKind;
use beacon_protocol::{Channel, Endpoint, encode_preamble};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Snapshot,
    Subscribe,
    Decide {
        id: u64,
        decision: Decision,
    },
    Acknowledge {
        session_id: String,
    },
    Snooze {
        session_id: Option<String>,
        minutes: u64,
    },
    Unsnooze {
        session_id: Option<String>,
    },
    Detail {
        session_id: String,
    },
    RefreshUsage,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Decision {
    Allow,
    AllowWithInput {
        input: Value,
    },
    AllowAlways {
        permissions: Vec<Value>,
    },
    Answer {
        answers: Map<String, Value>,
    },
    Deny {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    DenyAndStop {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Snapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<SessionDetail>,
}

impl Reply {
    pub fn ok() -> Reply {
        Reply {
            ok: true,
            error: None,
            snapshot: None,
            detail: None,
        }
    }

    pub fn failed(error: impl Into<String>) -> Reply {
        Reply {
            ok: false,
            error: Some(error.into()),
            ..Reply::ok()
        }
    }

    pub fn with_snapshot(snapshot: Snapshot) -> Reply {
        Reply {
            snapshot: Some(snapshot),
            ..Reply::ok()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub sessions: Vec<Session>,
    pub pending: Vec<PendingDecision>,
    pub resolved: Vec<ResolvedDecision>,
    pub snoozed_until_ms: Option<u64>,
    pub account_usage: Option<AccountUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDetail {
    pub session_id: String,
    pub timeline: Vec<TimelineEntry>,
    pub last_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub t_ms: u64,
    pub event: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingDecision {
    pub id: u64,
    pub session_id: String,
    pub agent_id: Option<String>,
    pub tool_name: String,
    pub tool_input: Value,
    pub permission_suggestions: Value,
    pub kind: PermissionKind,
    pub created_ms: u64,
    pub tool_use_id: Option<String>,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub expires_ms: u64,
    #[serde(default)]
    pub plan: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    Decided,
    AllowedInSession,
    DeniedInSession,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedDecision {
    pub id: u64,
    pub session_id: String,
    pub tool_name: String,
    pub resolution: Resolution,
    pub resolved_ms: u64,
}

pub struct ControlClient {
    pipe: BufReader<File>,
}

impl ControlClient {
    pub fn connect(home: &Path) -> io::Result<ControlClient> {
        let endpoint = Endpoint::read(home).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no usable endpoint file in {}", home.display()),
            )
        })?;
        let mut pipe = platform::open_pipe(&endpoint.pipe_name, Duration::from_secs(2))?;
        pipe.write_all(&encode_preamble(Channel::Control, &endpoint.token)?)?;
        Ok(ControlClient {
            pipe: BufReader::new(pipe),
        })
    }

    pub fn request(&mut self, request: &Request) -> io::Result<Reply> {
        let mut line = serde_json::to_vec(request)?;
        line.push(b'\n');
        self.pipe.get_mut().write_all(&line)?;
        self.read_reply()
    }

    pub fn subscribe(mut self) -> io::Result<Subscription> {
        let first = self.request(&Request::Subscribe)?;
        Ok(Subscription {
            client: self,
            first: Some(first),
        })
    }

    fn read_reply(&mut self) -> io::Result<Reply> {
        let mut line = String::new();
        if self.pipe.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the core closed the control connection",
            ));
        }
        Ok(serde_json::from_str(&line)?)
    }
}

pub struct Subscription {
    client: ControlClient,
    first: Option<Reply>,
}

impl Iterator for Subscription {
    type Item = io::Result<Reply>;

    fn next(&mut self) -> Option<io::Result<Reply>> {
        match self.first.take() {
            Some(first) => Some(Ok(first)),
            None => match self.client.read_reply() {
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => None,
                other => Some(other),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_have_the_documented_shapes() {
        let decide = Request::Decide {
            id: 3,
            decision: Decision::Deny {
                message: Some("no".to_string()),
            },
        };
        assert_eq!(
            serde_json::to_string(&decide).unwrap(),
            r#"{"op":"decide","id":3,"decision":{"type":"deny","message":"no"}}"#
        );
        let parsed: Request =
            serde_json::from_str(r#"{"op":"decide","id":1,"decision":{"type":"deny"}}"#).unwrap();
        assert_eq!(
            parsed,
            Request::Decide {
                id: 1,
                decision: Decision::Deny { message: None }
            }
        );
        assert_eq!(
            serde_json::to_string(&Request::Snapshot).unwrap(),
            r#"{"op":"snapshot"}"#
        );
        assert_eq!(
            serde_json::to_string(&Reply::failed("nope")).unwrap(),
            r#"{"ok":false,"error":"nope"}"#
        );
    }

    #[test]
    fn usage_snooze_and_detail_requests_have_the_documented_shapes() {
        let shapes = [
            (
                Request::Snooze {
                    session_id: Some("s".to_string()),
                    minutes: 30,
                },
                r#"{"op":"snooze","session_id":"s","minutes":30}"#,
            ),
            (
                Request::Snooze {
                    session_id: None,
                    minutes: 5,
                },
                r#"{"op":"snooze","session_id":null,"minutes":5}"#,
            ),
            (
                Request::Unsnooze {
                    session_id: Some("s".to_string()),
                },
                r#"{"op":"unsnooze","session_id":"s"}"#,
            ),
            (
                Request::Unsnooze { session_id: None },
                r#"{"op":"unsnooze","session_id":null}"#,
            ),
            (
                Request::Detail {
                    session_id: "s".to_string(),
                },
                r#"{"op":"detail","session_id":"s"}"#,
            ),
            (Request::RefreshUsage, r#"{"op":"refresh_usage"}"#),
        ];
        for (request, json) in shapes {
            assert_eq!(serde_json::to_string(&request).unwrap(), json);
            assert_eq!(serde_json::from_str::<Request>(json).unwrap(), request);
        }
    }
}
