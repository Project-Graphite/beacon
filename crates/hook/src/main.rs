mod platform;

use beacon_protocol::{
    DOCTOR_EVENT, Endpoint, FORWARDED_ENV, HookReply, HookRequest, VERSION, home_dir,
    is_waiting_event,
};
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CONNECT_BUDGET: Duration = Duration::from_millis(150);
const SEND_BUDGET: Duration = Duration::from_secs(2);
const REPLY_BUDGET: Duration = Duration::from_secs(1);

static STDOUT: Mutex<()> = Mutex::new(());

fn main() {
    std::panic::set_hook(Box::new(|_| std::process::exit(0)));
    let started = Instant::now();
    let Some(event) = std::env::args_os().nth(1) else {
        return;
    };
    let event = event.to_string_lossy().into_owned();
    let sent = start_watchdog(started, reply_budget(&event));
    if let Some(bytes) = forward(&event, started, sent) {
        let _writing = STDOUT.lock();
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(&bytes).and_then(|()| stdout.flush());
    }
}

fn reply_budget(event: &str) -> Option<Duration> {
    matches!(event, "Stop" | DOCTOR_EVENT).then_some(REPLY_BUDGET)
}

fn start_watchdog(started: Instant, reply_budget: Option<Duration>) -> Sender<()> {
    let (sent, on_sent) = mpsc::channel();
    std::thread::spawn(move || {
        let sent_in_time = on_sent
            .recv_timeout(SEND_BUDGET.saturating_sub(started.elapsed()))
            .is_ok();
        match (sent_in_time, reply_budget) {
            (true, None) => return,
            (true, Some(budget)) => std::thread::sleep(budget),
            (false, _) => {}
        }
        let _writing = STDOUT.lock();
        std::process::exit(0);
    });
    sent
}

fn forward(event: &str, started: Instant, sent: Sender<()>) -> Option<Vec<u8>> {
    let endpoint = Endpoint::read(&home_dir()?)?;
    let mut payload = Vec::new();
    std::io::stdin().read_to_end(&mut payload).ok()?;
    let mut pipe = platform::open_pipe(&endpoint.pipe_name, started + CONNECT_BUDGET)?;
    let request = HookRequest {
        meta: meta_json(event).into_bytes(),
        payload,
    };
    pipe.write_all(&request.encode(&endpoint.token).ok()?)
        .ok()?;
    sent.send(()).ok()?;
    if !is_waiting_event(event) {
        return None;
    }
    match HookReply::read_from(&mut pipe).ok()? {
        HookReply::Stdout(bytes) => Some(bytes),
        HookReply::NoDecision => None,
    }
}

fn meta_json(event: &str) -> String {
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    let mut json = format!("{{\"v\":{VERSION},\"event\":");
    push_json_string(&mut json, event);
    let _ = write!(
        json,
        ",\"pid\":{},\"t_ms\":{unix_ms},\"env\":{{",
        std::process::id()
    );
    let set_variables = FORWARDED_ENV
        .iter()
        .filter_map(|name| std::env::var_os(name).map(|value| (name, value)));
    for (index, (name, value)) in set_variables.enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_json_string(&mut json, name);
        json.push(':');
        push_json_string(&mut json, &value.to_string_lossy());
    }
    json.push_str("}}");
    json
}

fn push_json_string(json: &mut String, text: &str) {
    json.push('"');
    for character in text.chars() {
        match character {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            control if control < ' ' => {
                let _ = write!(json, "\\u{:04x}", control as u32);
            }
            other => json.push(other),
        }
    }
    json.push('"');
}

#[cfg(test)]
mod tests {
    use super::push_json_string;

    #[test]
    fn json_strings_are_escaped() {
        let mut json = String::new();
        push_json_string(&mut json, "a\"b\\c\nd\r\te\u{1}f\u{e9}");
        assert_eq!(json, "\"a\\\"b\\\\c\\nd\\r\\te\\u0001f\u{e9}\"");
    }
}
