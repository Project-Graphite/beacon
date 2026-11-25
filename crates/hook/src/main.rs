mod platform;

use beacon_protocol::{
    AUTOSTART_FILE, BACKGROUND_ARG, DOCTOR_EVENT, Endpoint, FORWARDED_ENV, HookReply, HookRequest,
    LAUNCHING_FILE, STOP_WAITER_EVENT, VERSION, home_dir, is_waiting_event,
};
use std::fmt::Write as _;
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CONNECT_BUDGET: Duration = Duration::from_millis(150);
const SEND_BUDGET: Duration = Duration::from_secs(2);
const REPLY_BUDGET: Duration = Duration::from_secs(1);
const LAUNCH_THROTTLE: Duration = Duration::from_secs(20);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(2);
const WAITER_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);

static STDOUT: Mutex<()> = Mutex::new(());

fn main() {
    std::panic::set_hook(Box::new(|_| std::process::exit(0)));
    let started = Instant::now();
    let Some(event) = std::env::args_os().nth(1) else {
        return;
    };
    let event = event.to_string_lossy().into_owned();
    let sent = start_watchdog(started, reply_budget(&event));
    match forward(&event, started, sent) {
        Some(HookReply::Stdout(bytes)) => {
            let _writing = STDOUT.lock();
            let mut stdout = std::io::stdout().lock();
            let _ = stdout.write_all(&bytes).and_then(|()| stdout.flush());
        }
        Some(HookReply::Rewake(bytes)) if event == STOP_WAITER_EVENT => {
            let mut stderr = std::io::stderr().lock();
            let _ = stderr.write_all(&bytes).and_then(|()| stderr.flush());
            std::process::exit(2);
        }
        _ => {}
    }
}

fn reply_budget(event: &str) -> Option<Duration> {
    matches!(event, "Stop" | "Elicitation" | DOCTOR_EVENT).then_some(REPLY_BUDGET)
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

fn forward(event: &str, started: Instant, sent: Sender<()>) -> Option<HookReply> {
    let home = home_dir()?;
    let Some(endpoint) = Endpoint::read(&home) else {
        start_app(&home, event);
        return None;
    };
    let mut payload = Vec::new();
    std::io::stdin().read_to_end(&mut payload).ok()?;
    let mut pipe = match platform::open_pipe(&endpoint.pipe_name, started + CONNECT_BUDGET) {
        Ok(pipe) => pipe,
        Err(error) => {
            if error.kind() == ErrorKind::NotFound {
                start_app(&home, event);
            }
            return None;
        }
    };
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
    match HookReply::read_from(&mut pipe) {
        Ok(reply) => Some(reply),
        Err(_) if event == STOP_WAITER_EVENT => Some(wait_for_next_core(&home, &request)),
        Err(_) => None,
    }
}

fn wait_for_next_core(home: &Path, request: &HookRequest) -> HookReply {
    let give_up = Instant::now() + WAITER_LIFETIME;
    let claude_pid = std::env::var("CLAUDE_PID")
        .ok()
        .and_then(|pid| pid.parse::<u32>().ok());
    loop {
        std::thread::sleep(RECONNECT_INTERVAL);
        if Instant::now() >= give_up || claude_pid.is_some_and(|pid| !platform::process_alive(pid))
        {
            return HookReply::NoDecision;
        }
        if let Some(reply) = register_again(home, request) {
            return reply;
        }
    }
}

fn register_again(home: &Path, request: &HookRequest) -> Option<HookReply> {
    let endpoint = Endpoint::read(home)?;
    let mut pipe =
        platform::open_pipe(&endpoint.pipe_name, Instant::now() + CONNECT_BUDGET).ok()?;
    pipe.write_all(&request.encode(&endpoint.token).ok()?)
        .ok()?;
    HookReply::read_from(&mut pipe).ok()
}

fn start_app(home: &Path, event: &str) {
    let wanted = matches!(event, "SessionStart" | "UserPromptSubmit")
        && std::env::var_os("CLAUDE_CODE_SESSION_ATTENDED").is_none_or(|attended| attended != "0");
    if !wanted {
        return;
    }
    let Ok(app) = std::fs::read_to_string(home.join(AUTOSTART_FILE)) else {
        return;
    };
    let throttle = home.join(LAUNCHING_FILE);
    let launched_lately = std::fs::metadata(&throttle)
        .and_then(|metadata| metadata.modified())
        .is_ok_and(|touched| touched.elapsed().is_ok_and(|age| age < LAUNCH_THROTTLE));
    if launched_lately {
        return;
    }
    let _ = std::fs::write(&throttle, std::process::id().to_string());
    platform::start_detached(Path::new(app.trim()), BACKGROUND_ARG);
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
