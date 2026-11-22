use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, SeekFrom};
use std::ops::AddAssign;
use std::path::{Path, PathBuf};

const PLAN_TOOL: &str = "ExitPlanMode";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageStatus {
    Ok,
    NoData,
    Unrecognized,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub context_tokens: u64,
    pub subagent_tokens: u64,
    pub reported_cost_usd: Option<f64>,
    pub model: Option<String>,
    pub updated_ms: u64,
    pub status: UsageStatus,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranscriptTitles {
    pub ai_title: Option<String>,
    pub custom_title: Option<String>,
    pub last_prompt: Option<String>,
}

pub fn claude_projects_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|profile| PathBuf::from(profile).join(".claude"))
        })
        .map(|dir| dir.join("projects"))
}

pub fn find_transcript(projects: &Path, session_id: &str) -> Option<PathBuf> {
    let file_name = format!("{session_id}.jsonl");
    std::fs::read_dir(projects)
        .ok()?
        .flatten()
        .map(|entry| entry.path().join(&file_name))
        .find(|candidate| candidate.is_file())
}

pub struct TranscriptReader {
    transcript: PathBuf,
    subagents_dir: PathBuf,
    main: FileTally,
    subagents: BTreeMap<PathBuf, FileTally>,
}

impl TranscriptReader {
    pub fn new(session_id: &str, transcript: &Path) -> TranscriptReader {
        TranscriptReader {
            transcript: transcript.to_path_buf(),
            subagents_dir: transcript
                .parent()
                .unwrap_or(Path::new(""))
                .join(session_id)
                .join("subagents"),
            main: FileTally::default(),
            subagents: BTreeMap::new(),
        }
    }

    pub fn transcript(&self) -> &Path {
        &self.transcript
    }

    pub fn take_titles(&mut self) -> TranscriptTitles {
        std::mem::take(&mut self.main.titles)
    }

    pub fn read(&mut self, now_ms: u64) -> io::Result<SessionUsage> {
        self.main.read_new(&self.transcript)?;
        for path in subagent_files(&self.subagents_dir)? {
            self.subagents
                .entry(path.clone())
                .or_default()
                .read_new(&path)?;
        }
        let main = &self.main;
        let context = main.last_usage.unwrap_or_default();
        Ok(SessionUsage {
            input_tokens: main.totals.input,
            output_tokens: main.totals.output,
            cache_creation_tokens: main.totals.cache_creation,
            cache_read_tokens: main.totals.cache_read,
            context_tokens: context.input + context.cache_creation + context.cache_read,
            subagent_tokens: self
                .subagents
                .values()
                .map(|tally| tally.totals.sum())
                .sum(),
            reported_cost_usd: main.reported_cost_usd,
            model: main.model.clone(),
            updated_ms: now_ms,
            status: match (main.assistant_lines, main.usage_lines) {
                (0, _) => UsageStatus::NoData,
                (_, 0) => UsageStatus::Unrecognized,
                _ => UsageStatus::Ok,
            },
        })
    }
}

pub fn find_plan(transcript: &Path, tool_use_id: Option<&str>) -> io::Result<Option<String>> {
    let mut last = None;
    for line in BufReader::new(File::open(transcript)?).split(b'\n') {
        let line = line?;
        if !line
            .windows(PLAN_TOOL.len())
            .any(|window| window == PLAN_TOOL.as_bytes())
        {
            continue;
        }
        let Ok(line) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if line["type"] != "assistant" {
            continue;
        }
        for block in line["message"]["content"].as_array().into_iter().flatten() {
            let Some(plan) = block["input"]["plan"]
                .as_str()
                .filter(|plan| !plan.is_empty())
            else {
                continue;
            };
            if block["type"] != "tool_use" || block["name"] != PLAN_TOOL {
                continue;
            }
            match tool_use_id {
                Some(id) if block["id"] == id => return Ok(Some(plan.to_string())),
                Some(_) => {}
                None => last = Some(plan.to_string()),
            }
        }
    }
    Ok(last)
}

fn subagent_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    match std::fs::read_dir(dir) {
        Ok(entries) => Ok(entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<PathBuf>>>()?
            .into_iter()
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "jsonl")
            })
            .collect()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Tokens {
    input: u64,
    output: u64,
    cache_creation: u64,
    cache_read: u64,
}

impl Tokens {
    fn sum(self) -> u64 {
        self.input + self.output + self.cache_creation + self.cache_read
    }
}

impl AddAssign for Tokens {
    fn add_assign(&mut self, other: Tokens) {
        self.input += other.input;
        self.output += other.output;
        self.cache_creation += other.cache_creation;
        self.cache_read += other.cache_read;
    }
}

#[derive(Default)]
struct FileTally {
    offset: u64,
    seen: HashSet<String>,
    totals: Tokens,
    last_usage: Option<Tokens>,
    model: Option<String>,
    reported_cost_usd: Option<f64>,
    assistant_lines: u64,
    usage_lines: u64,
    titles: TranscriptTitles,
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: Option<String>,
    message: Option<Message>,
    #[serde(rename = "totalCostUSD")]
    total_cost_usd: Option<f64>,
    #[serde(rename = "aiTitle")]
    ai_title: Option<String>,
    #[serde(rename = "customTitle")]
    custom_title: Option<String>,
    #[serde(rename = "lastPrompt")]
    last_prompt: Option<String>,
}

#[derive(Deserialize)]
struct LineKind {
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

impl From<Usage> for Tokens {
    fn from(usage: Usage) -> Tokens {
        Tokens {
            input: usage.input_tokens.unwrap_or(0),
            output: usage.output_tokens.unwrap_or(0),
            cache_creation: usage.cache_creation_input_tokens.unwrap_or(0),
            cache_read: usage.cache_read_input_tokens.unwrap_or(0),
        }
    }
}

impl FileTally {
    fn read_new(&mut self, path: &Path) -> io::Result<()> {
        let mut file = File::open(path)?;
        if file.metadata()?.len() < self.offset {
            *self = FileTally::default();
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut reader = BufReader::new(file);
        let mut line = Vec::new();
        loop {
            line.clear();
            let read = reader.read_until(b'\n', &mut line)?;
            if line.last() != Some(&b'\n') {
                return Ok(());
            }
            self.offset += read as u64;
            self.count(&line);
        }
    }

    fn count(&mut self, line: &[u8]) {
        let Ok(parsed) = serde_json::from_slice::<Line>(line) else {
            if serde_json::from_slice::<LineKind>(line)
                .is_ok_and(|line| line.kind.as_deref() == Some("assistant"))
            {
                self.assistant_lines += 1;
            }
            return;
        };
        match parsed.kind.as_deref() {
            Some("assistant") => {
                self.assistant_lines += 1;
                if let Some(Message {
                    id,
                    model,
                    usage: Some(usage),
                }) = parsed.message
                {
                    self.usage_lines += 1;
                    self.add(id, model, usage.into());
                }
            }
            Some("cost-state") if parsed.total_cost_usd.is_some() => {
                self.reported_cost_usd = parsed.total_cost_usd;
            }
            Some("ai-title") if parsed.ai_title.is_some() => self.titles.ai_title = parsed.ai_title,
            Some("custom-title") if parsed.custom_title.is_some() => {
                self.titles.custom_title = parsed.custom_title;
            }
            Some("last-prompt") if parsed.last_prompt.is_some() => {
                self.titles.last_prompt = parsed.last_prompt;
            }
            _ => {}
        }
    }

    fn add(&mut self, id: Option<String>, model: Option<String>, tokens: Tokens) {
        if id.is_some_and(|id| !self.seen.insert(id)) {
            return;
        }
        self.totals += tokens;
        if tokens.sum() > 0 {
            self.last_usage = Some(tokens);
            self.model = model.or(self.model.take());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "beacon-usage-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn assistant(id: &str, model: &str, usage: [u64; 4]) -> String {
        json!({
            "type": "assistant",
            "message": {
                "id": id,
                "model": model,
                "role": "assistant",
                "content": [{"type": "text", "text": "synthetic"}],
                "usage": {
                    "input_tokens": usage[0],
                    "output_tokens": usage[1],
                    "cache_creation_input_tokens": usage[2],
                    "cache_read_input_tokens": usage[3],
                    "server_tool_use": {"web_search_requests": 0},
                },
            },
        })
        .to_string()
            + "\n"
    }

    fn user() -> String {
        json!({"type": "user", "message": {"role": "user", "content": "synthetic prompt"}})
            .to_string()
            + "\n"
    }

    fn append(path: &Path, text: &str) {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }

    fn totals(usage: &SessionUsage) -> [u64; 6] {
        [
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_creation_tokens,
            usage.cache_read_tokens,
            usage.context_tokens,
            usage.subagent_tokens,
        ]
    }

    #[test]
    fn repeated_message_ids_count_once_and_the_last_message_sets_the_context() {
        let dir = TempDir::new();
        let transcript = dir.0.join("s1.jsonl");
        append(
            &transcript,
            &[
                user(),
                assistant("msg_a", "model-a", [10, 5, 100, 1000]),
                assistant("msg_a", "model-a", [10, 5, 100, 1000]),
                assistant("msg_a", "model-a", [10, 5, 100, 1000]),
                user(),
                assistant("msg_b", "model-b", [2, 7, 30, 1100]),
                assistant("msg_b", "model-b", [2, 7, 30, 1100]),
                assistant("msg_synthetic", "<synthetic>", [0, 0, 0, 0]),
                json!({"type": "cost-state", "totalCostUSD": 0.42, "models": {}}).to_string()
                    + "\n",
            ]
            .concat(),
        );
        let usage = TranscriptReader::new("s1", &transcript).read(7).unwrap();
        assert_eq!(totals(&usage), [12, 12, 130, 2100, 1132, 0]);
        assert_eq!(usage.model.as_deref(), Some("model-b"));
        assert_eq!(usage.reported_cost_usd, Some(0.42));
        assert_eq!(usage.updated_ms, 7);
        assert_eq!(usage.status, UsageStatus::Ok);
    }

    #[test]
    fn reading_is_incremental_and_waits_for_a_complete_line() {
        let dir = TempDir::new();
        let transcript = dir.0.join("s2.jsonl");
        let line = assistant("msg_b", "m", [1, 1, 1, 1]);
        append(
            &transcript,
            &(assistant("msg_a", "m", [1, 2, 3, 4]) + &line[..line.len() / 2]),
        );
        let mut reader = TranscriptReader::new("s2", &transcript);
        assert_eq!(totals(&reader.read(0).unwrap()), [1, 2, 3, 4, 8, 0]);
        append(&transcript, &line[line.len() / 2..]);
        assert_eq!(totals(&reader.read(0).unwrap()), [2, 3, 4, 5, 3, 0]);
        append(&transcript, &assistant("msg_b", "m", [1, 1, 1, 1]));
        assert_eq!(totals(&reader.read(0).unwrap()), [2, 3, 4, 5, 3, 0]);
    }

    #[test]
    fn a_shrunken_file_is_read_again_from_the_start() {
        let dir = TempDir::new();
        let transcript = dir.0.join("s3.jsonl");
        append(
            &transcript,
            &[
                assistant("msg_a", "m", [5, 5, 5, 5]),
                assistant("msg_b", "m", [5, 5, 5, 5]),
            ]
            .concat(),
        );
        let mut reader = TranscriptReader::new("s3", &transcript);
        assert_eq!(reader.read(0).unwrap().input_tokens, 10);
        std::fs::write(&transcript, assistant("msg_c", "m", [1, 0, 0, 0])).unwrap();
        assert_eq!(totals(&reader.read(0).unwrap()), [1, 0, 0, 0, 1, 0]);
    }

    #[test]
    fn subagent_files_add_a_deduplicated_grand_total() {
        let dir = TempDir::new();
        let transcript = dir.0.join("s4.jsonl");
        append(&transcript, &assistant("msg_main", "m", [1, 1, 1, 1]));
        let subagents = dir.0.join("s4").join("subagents");
        std::fs::create_dir_all(&subagents).unwrap();
        append(
            &subagents.join("agent-1.jsonl"),
            &[
                assistant("msg_x", "m", [10, 20, 30, 40]),
                assistant("msg_x", "m", [10, 20, 30, 40]),
            ]
            .concat(),
        );
        append(
            &subagents.join("agent-2.jsonl"),
            &assistant("msg_y", "m", [1, 2, 3, 4]),
        );
        append(
            &subagents.join("notes.txt"),
            &assistant("msg_z", "m", [9, 9, 9, 9]),
        );
        let mut reader = TranscriptReader::new("s4", &transcript);
        assert_eq!(totals(&reader.read(0).unwrap()), [1, 1, 1, 1, 3, 110]);
        append(
            &subagents.join("agent-2.jsonl"),
            &assistant("msg_w", "m", [1, 0, 0, 0]),
        );
        assert_eq!(reader.read(0).unwrap().subagent_tokens, 111);
    }

    #[test]
    fn no_assistant_line_is_no_data_and_assistant_lines_without_usage_are_unrecognized() {
        let dir = TempDir::new();
        let empty = dir.0.join("empty.jsonl");
        append(&empty, &user());
        let usage = TranscriptReader::new("empty", &empty).read(0).unwrap();
        assert_eq!(usage.status, UsageStatus::NoData);
        assert_eq!(usage.model, None);
        let changed = dir.0.join("changed.jsonl");
        append(
            &changed,
            &[
                json!({"type": "assistant", "message": {"id": "m1", "tokens": {"in": 3}}})
                    .to_string(),
                "\n".to_string(),
                json!({"type": "assistant", "message": "now a string"}).to_string(),
                "\n".to_string(),
            ]
            .concat(),
        );
        let usage = TranscriptReader::new("changed", &changed).read(0).unwrap();
        assert_eq!(usage.status, UsageStatus::Unrecognized);
        assert_eq!(totals(&usage), [0; 6]);
    }

    fn plan_call(id: &str, plan: &str) -> String {
        json!({
            "type": "assistant",
            "message": {
                "id": format!("msg_{id}"),
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Here is the plan."},
                    {"type": "tool_use", "id": id, "name": "ExitPlanMode", "input": {"plan": plan}},
                ],
            },
        })
        .to_string()
            + "\n"
    }

    #[test]
    fn the_plan_is_found_by_its_tool_use_id() {
        let dir = TempDir::new();
        let transcript = dir.0.join("p1.jsonl");
        append(
            &transcript,
            &[
                user(),
                plan_call("toolu_a", "# First plan"),
                plan_call("toolu_b", "# Second plan"),
                json!({"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_b", "content": "ExitPlanMode"}]}}).to_string() + "\n",
            ]
            .concat(),
        );
        assert_eq!(
            find_plan(&transcript, Some("toolu_a")).unwrap().as_deref(),
            Some("# First plan")
        );
        assert_eq!(
            find_plan(&transcript, Some("toolu_b")).unwrap().as_deref(),
            Some("# Second plan")
        );
        assert_eq!(find_plan(&transcript, Some("toolu_c")).unwrap(), None);
    }

    #[test]
    fn without_a_tool_use_id_the_last_plan_call_is_used() {
        let dir = TempDir::new();
        let transcript = dir.0.join("p2.jsonl");
        append(
            &transcript,
            &[
                plan_call("toolu_a", "# Old plan"),
                assistant("msg_x", "m", [1, 1, 1, 1]),
                plan_call("toolu_b", "# New plan"),
                json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "toolu_c", "name": "ExitPlanMode", "input": {}}]}}).to_string() + "\n",
            ]
            .concat(),
        );
        assert_eq!(
            find_plan(&transcript, None).unwrap().as_deref(),
            Some("# New plan")
        );
    }

    #[test]
    fn a_transcript_without_a_plan_call_has_no_plan() {
        let dir = TempDir::new();
        let transcript = dir.0.join("p3.jsonl");
        append(
            &transcript,
            &[
                user(),
                assistant("msg_a", "m", [1, 1, 1, 1]),
                json!({"type": "user", "message": {"role": "user", "content": "mention ExitPlanMode in text"}}).to_string() + "\n",
                json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "toolu_x", "name": "Bash", "input": {"plan": "not a plan", "command": "ExitPlanMode"}}]}}).to_string() + "\n",
                "{ExitPlanMode not json\n".to_string(),
            ]
            .concat(),
        );
        assert_eq!(find_plan(&transcript, None).unwrap(), None);
        assert_eq!(find_plan(&transcript, Some("toolu_x")).unwrap(), None);
        assert!(find_plan(&dir.0.join("gone.jsonl"), None).is_err());
    }

    #[test]
    fn a_plan_written_after_the_first_look_is_found_on_a_retry() {
        let dir = TempDir::new();
        let transcript = dir.0.join("p4.jsonl");
        let call = plan_call("toolu_late", "# Late plan");
        append(&transcript, &(user() + &call[..call.len() / 2]));
        assert_eq!(find_plan(&transcript, Some("toolu_late")).unwrap(), None);
        append(&transcript, &call[call.len() / 2..]);
        assert_eq!(
            find_plan(&transcript, Some("toolu_late"))
                .unwrap()
                .as_deref(),
            Some("# Late plan")
        );
    }

    fn line(value: serde_json::Value) -> String {
        value.to_string() + "\n"
    }

    #[test]
    fn the_latest_titles_and_last_prompt_are_handed_over_once() {
        let dir = TempDir::new();
        let transcript = dir.0.join("t1.jsonl");
        append(
            &transcript,
            &[
                line(json!({"type": "ai-title", "aiTitle": "First title", "sessionId": "t1"})),
                user(),
                line(json!({"type": "ai-title", "aiTitle": "Second title", "sessionId": "t1"})),
                line(json!({"type": "custom-title", "customTitle": "Named", "sessionId": "t1"})),
                line(
                    json!({"type": "last-prompt", "lastPrompt": "do the thing", "leafUuid": "u", "sessionId": "t1"}),
                ),
                line(json!({"type": "ai-title", "sessionId": "t1"})),
                assistant("msg_a", "m", [1, 1, 1, 1]),
            ]
            .concat(),
        );
        let mut reader = TranscriptReader::new("t1", &transcript);
        assert_eq!(reader.read(0).unwrap().input_tokens, 1);
        assert_eq!(
            reader.take_titles(),
            TranscriptTitles {
                ai_title: Some("Second title".to_string()),
                custom_title: Some("Named".to_string()),
                last_prompt: Some("do the thing".to_string()),
            }
        );
        assert_eq!(reader.take_titles(), TranscriptTitles::default());
        append(
            &transcript,
            &line(json!({"type": "last-prompt", "lastPrompt": "next", "sessionId": "t1"})),
        );
        reader.read(0).unwrap();
        assert_eq!(
            reader.take_titles(),
            TranscriptTitles {
                last_prompt: Some("next".to_string()),
                ..TranscriptTitles::default()
            }
        );
    }

    #[test]
    fn a_transcript_is_found_by_its_session_id_in_any_project_folder() {
        let dir = TempDir::new();
        for project in ["C--work-a", "C--work-b"] {
            std::fs::create_dir_all(dir.0.join(project)).unwrap();
        }
        let transcript = dir.0.join("C--work-b").join("s9.jsonl");
        append(&transcript, &user());
        assert_eq!(find_transcript(&dir.0, "s9"), Some(transcript));
        assert_eq!(find_transcript(&dir.0, "s8"), None);
        assert_eq!(find_transcript(&dir.0.join("missing"), "s9"), None);
    }

    #[test]
    fn a_missing_transcript_is_an_error() {
        let dir = TempDir::new();
        assert!(
            TranscriptReader::new("gone", &dir.0.join("gone.jsonl"))
                .read(0)
                .is_err()
        );
    }
}
