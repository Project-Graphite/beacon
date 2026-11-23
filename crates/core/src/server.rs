use crate::account::{self, AccountSources, UsageProber};
use crate::config::Config;
use crate::control::{Reply, Request};
use crate::engine::{Engine, HookOutcome, ProcessExit};
use crate::focus;
use crate::lister::SessionLister;
use crate::log::{Logger, now_ms};
use crate::platform::{self, InstanceLock, PIPE_PREFIX, PipeSecurity};
use crate::usage::{TranscriptReader, find_plan, find_transcript};
use beacon_protocol::{
    Channel, ENDPOINT_FILE, Endpoint, HookReply, MAX_FRAME_LEN, TOKEN_LEN, channel_for, frame_len,
    tokens_match,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::io::{self, Write};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::{JoinError, JoinSet, spawn_blocking};
use tokio::time::{Instant, sleep, sleep_until, timeout};

const LISTENERS: usize = 2;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const PERSIST_DEBOUNCE: Duration = Duration::from_secs(2);
const SUBSCRIBE_INTERVAL: Duration = Duration::from_millis(250);
const STALL_CHECK_INTERVAL: Duration = Duration::from_secs(30);
const USAGE_READ_INTERVAL: Duration = Duration::from_secs(2);
const CACHE_CHECK_INTERVAL: Duration = Duration::from_secs(60);
const RECONCILE_SPACING: Duration = Duration::from_secs(15);
const PLAN_RETRY_DELAYS: [Duration; 5] = [
    Duration::ZERO,
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_millis(1000),
    Duration::from_millis(1250),
];

pub struct CoreOptions {
    pub home: PathBuf,
    pub lister: Box<dyn SessionLister>,
    pub config: Config,
}

pub struct Core;

pub struct CoreHandle {
    shutdown: Arc<watch::Sender<bool>>,
    thread: JoinHandle<io::Result<()>>,
    endpoint: Endpoint,
}

impl Core {
    pub fn start(options: CoreOptions) -> io::Result<CoreHandle> {
        Core::start_with(options, AccountSources::default())
    }

    pub fn start_with(options: CoreOptions, account: AccountSources) -> io::Result<CoreHandle> {
        let shutdown = Arc::new(watch::channel(false).0);
        let (ready, started) = std::sync::mpsc::channel();
        let thread_shutdown = shutdown.clone();
        let thread = std::thread::Builder::new()
            .name("beacon-core".to_string())
            .spawn(move || run(options, account, thread_shutdown, ready))?;
        match started.recv() {
            Ok(Ok(endpoint)) => Ok(CoreHandle {
                shutdown,
                thread,
                endpoint,
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(join(thread)
                .err()
                .unwrap_or_else(|| io::Error::other("the core stopped while starting"))),
        }
    }
}

impl CoreHandle {
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn shutdown(self) -> io::Result<()> {
        self.shutdown.send_replace(true);
        join(self.thread)
    }

    pub fn wait(self) -> io::Result<()> {
        join(self.thread)
    }
}

fn join(thread: JoinHandle<io::Result<()>>) -> io::Result<()> {
    thread
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("the core thread panicked")))
}

struct Shared {
    engine: RefCell<Engine>,
    home: PathBuf,
    token: [u8; TOKEN_LEN],
    pipe_name: String,
    security: PipeSecurity,
    shutdown: Arc<watch::Sender<bool>>,
    prober: Option<Box<dyn UsageProber>>,
}

struct Started {
    shared: Rc<Shared>,
    listeners: Vec<NamedPipeServer>,
    exits: mpsc::UnboundedReceiver<ProcessExit>,
    endpoint: Endpoint,
    _lock: InstanceLock,
}

fn run(
    options: CoreOptions,
    account: AccountSources,
    shutdown: Arc<watch::Sender<bool>>,
    ready: std::sync::mpsc::Sender<io::Result<Endpoint>>,
) -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let started = {
        let _context = runtime.enter();
        start(&options, account.prober, shutdown.clone())
    };
    let started = match started {
        Ok(started) => started,
        Err(error) => {
            let _ = ready.send(Err(error));
            return Ok(());
        }
    };
    let _ = ready.send(Ok(started.endpoint));
    let shared = started.shared;
    let local = tokio::task::LocalSet::new();
    let result = local.block_on(&runtime, async {
        let mut tasks = JoinSet::new();
        for listener in started.listeners {
            tasks.spawn_local(listen(shared.clone(), listener));
        }
        tasks.spawn_local(watch_process_exits(shared.clone(), started.exits));
        tasks.spawn_local(reconcile(
            shared.clone(),
            options.lister,
            options.config.reconcile_seconds,
            account.claude_json,
        ));
        tasks.spawn_local(check_stalls(shared.clone()));
        tasks.spawn_local(expire_snoozes(shared.clone()));
        tasks.spawn_local(read_usage(shared.clone()));
        tasks.spawn_local(read_plans(shared.clone()));
        tasks.spawn_local(find_transcripts(shared.clone(), account.projects_dir));
        tasks.spawn_local(persist(shared.clone(), options.home.clone()));
        let mut shutdown_requested = shutdown.subscribe();
        loop {
            tokio::select! {
                _ = shutdown_requested.wait_for(|requested| *requested) => break Ok(()),
                Some(finished) = tasks.join_next() => match finished {
                    Err(error) if error.is_panic() => log_panic(&shared, &error),
                    finished => break finished.map_err(io::Error::other).and_then(|result| result),
                },
            }
        }
    });
    drop(local);
    let engine = shared.engine.borrow();
    match &result {
        Ok(()) => engine.log.info("core stopped"),
        Err(error) => engine.log.error(format_args!("core failed: {error}")),
    }
    write_atomic(&options.home.join("state.json"), &engine.persisted_state()?)?;
    match std::fs::remove_file(options.home.join(ENDPOINT_FILE)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    result
}

fn start(
    options: &CoreOptions,
    prober: Option<Box<dyn UsageProber>>,
    shutdown: Arc<watch::Sender<bool>>,
) -> io::Result<Started> {
    std::fs::create_dir_all(&options.home)?;
    let lock = InstanceLock::acquire(&options.home)?;
    let log = Logger::open(
        &options.home.join("logs").join("core.log"),
        options.config.log_level,
    )?;
    let state_path = options.home.join("state.json");
    let (sessions, global_snooze) = match std::fs::read(&state_path) {
        Ok(bytes) => match Engine::load_sessions(&bytes)
            .and_then(|sessions| Ok((sessions, Engine::load_global_snooze(&bytes)?)))
        {
            Ok(state) => state,
            Err(error) => {
                log.error(format_args!(
                    "state.json cannot be read, starting with no sessions: {error}"
                ));
                if let Err(error) =
                    std::fs::rename(&state_path, state_path.with_extension("json.bad"))
                {
                    log.warn(format_args!(
                        "cannot keep state.json as state.json.bad: {error}"
                    ));
                }
                (Vec::new(), None)
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => (Vec::new(), None),
        Err(error) => return Err(error),
    };
    let endpoint = Endpoint {
        pipe_name: format!("{PIPE_PREFIX}{}", hex(&platform::random_bytes::<8>()?)),
        token: hex(&platform::random_bytes::<32>()?),
    };
    let security = PipeSecurity::current_user_only()?;
    let listeners = (0..LISTENERS)
        .map(|index| create_instance(&security, &endpoint.pipe_name, index == 0))
        .collect::<io::Result<Vec<_>>>()?;
    write_atomic(
        &options.home.join(ENDPOINT_FILE),
        endpoint.to_file_text().as_bytes(),
    )?;
    log.info(format_args!(
        "core started on {} with {} sessions",
        endpoint.pipe_name,
        sessions.len()
    ));
    let (mut engine, exits) = Engine::new(options.config.clone(), log, sessions);
    engine
        .snooze(None, global_snooze)
        .map_err(io::Error::other)?;
    let mut token = [0u8; TOKEN_LEN];
    token.copy_from_slice(endpoint.token.as_bytes());
    Ok(Started {
        shared: Rc::new(Shared {
            engine: RefCell::new(engine),
            home: options.home.clone(),
            token,
            pipe_name: endpoint.pipe_name.clone(),
            security,
            shutdown,
            prober,
        }),
        listeners,
        exits,
        endpoint,
        _lock: lock,
    })
}

fn create_instance(
    security: &PipeSecurity,
    pipe_name: &str,
    first: bool,
) -> io::Result<NamedPipeServer> {
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first)
        .reject_remote_clients(true);
    unsafe { options.create_with_security_attributes_raw(pipe_name, security.attributes()) }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)
}

async fn listen(shared: Rc<Shared>, mut server: NamedPipeServer) -> io::Result<()> {
    loop {
        let connected = server.connect().await;
        let next = create_instance(&shared.security, &shared.pipe_name, false)?;
        let client = std::mem::replace(&mut server, next);
        if let Err(error) = connected {
            shared
                .engine
                .borrow()
                .log
                .debug(format_args!("pipe connect failed: {error}"));
            continue;
        }
        let connection = shared.clone();
        let served = tokio::task::spawn_local(async move {
            if let Err(error) = serve(&connection, client).await {
                connection
                    .engine
                    .borrow()
                    .log
                    .debug(format_args!("connection dropped: {error}"));
            }
        });
        let watcher = shared.clone();
        tokio::task::spawn_local(async move {
            if let Err(error) = served.await
                && error.is_panic()
            {
                log_panic(&watcher, &error);
            }
        });
    }
}

fn log_panic(shared: &Shared, error: &JoinError) {
    shared
        .engine
        .borrow()
        .log
        .error(format_args!("a task panicked and was dropped: {error}"));
}

async fn serve(shared: &Shared, mut pipe: NamedPipeServer) -> io::Result<()> {
    let channel = timeout(REQUEST_TIMEOUT, read_preamble(shared, &mut pipe))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "no preamble"))??;
    match channel {
        Channel::Hook => serve_hook(shared, pipe).await,
        Channel::Control => serve_control(shared, pipe).await,
    }
}

async fn read_preamble(shared: &Shared, pipe: &mut NamedPipeServer) -> io::Result<Channel> {
    let mut magic = [0u8; 4];
    pipe.read_exact(&mut magic).await?;
    let channel = channel_for(magic)?;
    let mut token = [0u8; TOKEN_LEN];
    pipe.read_exact(&mut token).await?;
    if !tokens_match(&shared.token, &token) {
        shared
            .engine
            .borrow()
            .log
            .warn("rejected a connection with a bad token");
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "bad token"));
    }
    Ok(channel)
}

async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut prefix = [0u8; 4];
    reader.read_exact(&mut prefix).await?;
    let mut bytes = vec![0u8; frame_len(prefix)?];
    reader.read_exact(&mut bytes).await?;
    Ok(bytes)
}

async fn serve_hook(shared: &Shared, mut pipe: NamedPipeServer) -> io::Result<()> {
    let (meta, payload) = timeout(REQUEST_TIMEOUT, async {
        Ok::<_, io::Error>((read_frame(&mut pipe).await?, read_frame(&mut pipe).await?))
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "incomplete hook request"))??;
    let outcome = shared.engine.borrow_mut().handle_hook(&meta, &payload);
    match outcome {
        HookOutcome::Done => Ok(()),
        HookOutcome::Reply(reply) => send_reply(&mut pipe, reply).await,
        HookOutcome::Wait(id, decided) => {
            if !reply_when_ready(&mut pipe, decided).await? {
                shared.engine.borrow_mut().helper_gone(id);
            }
            Ok(())
        }
        HookOutcome::Waiter(id, woken) => {
            if !reply_when_ready(&mut pipe, woken).await? {
                shared.engine.borrow_mut().waiter_gone(id);
            }
            Ok(())
        }
    }
}

async fn reply_when_ready(
    pipe: &mut NamedPipeServer,
    ready: oneshot::Receiver<HookReply>,
) -> io::Result<bool> {
    let mut probe = [0u8; 1];
    let waited = tokio::select! {
        reply = ready => Some(reply),
        _ = pipe.read(&mut probe) => None,
    };
    match waited {
        Some(Ok(reply)) => send_reply(pipe, reply).await.map(|()| true),
        Some(Err(_)) => Ok(true),
        None => Ok(false),
    }
}

async fn send_reply(pipe: &mut NamedPipeServer, reply: HookReply) -> io::Result<()> {
    pipe.write_all(&reply.encode()?).await?;
    let mut probe = [0u8; 1];
    let _ = timeout(CLOSE_TIMEOUT, pipe.read(&mut probe)).await;
    Ok(())
}

fn decide_refusal(shared: &Shared, client_pid: &io::Result<u32>) -> Option<String> {
    let pid = match client_pid {
        Ok(pid) => *pid,
        Err(error) => return Some(format!("cannot identify the control client: {error}")),
    };
    let sessions = shared.engine.borrow().live_claude_pids();
    match platform::processes() {
        Ok(processes) => {
            focus::descends_from(pid, &sessions, &processes, platform::process_created)
                .then(|| "decisions cannot come from inside a Claude session".to_string())
        }
        Err(error) => Some(format!("cannot inspect the running processes: {error}")),
    }
}

async fn serve_control(shared: &Shared, pipe: NamedPipeServer) -> io::Result<()> {
    let client_pid = platform::pipe_client_pid(pipe.as_raw_handle());
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut reader = BufReader::new(reader);
    while let Some(line) = read_line(&mut reader).await? {
        let reply = match serde_json::from_slice::<Request>(&line) {
            Ok(Request::Subscribe) => return stream_snapshots(shared, &mut writer).await,
            Ok(Request::Shutdown) => {
                write_reply(&mut writer, &Reply::ok()).await?;
                shared
                    .engine
                    .borrow()
                    .log
                    .info("shutdown requested over the control channel");
                shared.shutdown.send_replace(true);
                return Ok(());
            }
            Ok(Request::RefreshUsage) => refresh_usage(shared).await,
            Ok(Request::SetConfig {
                stall_minutes,
                answer_unattended,
                clear_finished_minutes,
            }) => shared
                .engine
                .borrow_mut()
                .set_config(
                    &shared.home,
                    stall_minutes,
                    answer_unattended,
                    clear_finished_minutes,
                )
                .map_or_else(Reply::failed, |()| Reply::ok()),
            Ok(request @ Request::Decide { .. }) => match decide_refusal(shared, &client_pid) {
                Some(refusal) => Reply::failed(refusal),
                None => shared.engine.borrow_mut().control(request),
            },
            Ok(request) => shared.engine.borrow_mut().control(request),
            Err(error) => Reply::failed(format!("bad request: {error}")),
        };
        write_reply(&mut writer, &reply).await?;
    }
    Ok(())
}

async fn read_line<R: AsyncBufReadExt + Unpin>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    if reader
        .take(MAX_FRAME_LEN as u64 + 1)
        .read_until(b'\n', &mut line)
        .await?
        == 0
    {
        return Ok(None);
    }
    if line.len() > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "control line exceeds the maximum size",
        ));
    }
    Ok(Some(line))
}

async fn write_reply<W: AsyncWrite + Unpin>(writer: &mut W, reply: &Reply) -> io::Result<()> {
    let mut line = serde_json::to_vec(reply)?;
    line.push(b'\n');
    writer.write_all(&line).await
}

async fn stream_snapshots<W: AsyncWrite + Unpin>(
    shared: &Shared,
    writer: &mut W,
) -> io::Result<()> {
    let mut changes = shared.engine.borrow().watch_changes();
    loop {
        changes.borrow_and_update();
        let (snapshot, sent_needs) = {
            let engine = shared.engine.borrow();
            (engine.snapshot(), engine.needs_user())
        };
        write_reply(writer, &Reply::with_snapshot(snapshot)).await?;
        let next_allowed = Instant::now() + SUBSCRIBE_INTERVAL;
        if changes.changed().await.is_err() {
            return Ok(());
        }
        loop {
            if shared.engine.borrow().needs_user() != sent_needs {
                break;
            }
            tokio::select! {
                () = sleep_until(next_allowed) => break,
                changed = changes.changed() => {
                    if changed.is_err() {
                        return Ok(());
                    }
                }
            }
        }
    }
}

async fn watch_process_exits(
    shared: Rc<Shared>,
    mut exits: mpsc::UnboundedReceiver<ProcessExit>,
) -> io::Result<()> {
    while let Some((session_id, pid)) = exits.recv().await {
        shared.engine.borrow_mut().process_exited(&session_id, pid);
    }
    Ok(())
}

async fn refresh_usage(shared: &Shared) -> Reply {
    let Some(prober) = &shared.prober else {
        return Reply::failed("this core has no usage prober");
    };
    let probed = prober.probe().await.and_then(|stdout| {
        account::parse_probe(&stdout, now_ms()).ok_or_else(|| {
            io::Error::other("the probe output had no rate_limit_event with unifiedWindows")
        })
    });
    match probed {
        Ok(usage) => {
            shared.engine.borrow_mut().offer_account_usage(usage);
            Reply::ok()
        }
        Err(error) => Reply::failed(format!("usage refresh failed: {error}")),
    }
}

async fn reconcile(
    shared: Rc<Shared>,
    lister: Box<dyn SessionLister>,
    reconcile_seconds: u64,
    claude_json: Option<PathBuf>,
) -> io::Result<()> {
    let interval = Duration::from_secs(reconcile_seconds.max(1));
    let cache_interval = interval.min(CACHE_CHECK_INTERVAL);
    let mut changes = shared.engine.borrow().watch_changes();
    let mut cache = CacheWatch {
        path: claude_json,
        modified: None,
        warned: false,
    };
    cache.refresh(&shared).await?;
    loop {
        shared.engine.borrow_mut().take_reconcile_wanted();
        let listing_started_ms = now_ms();
        match lister.list().await {
            Ok(listed) => shared
                .engine
                .borrow_mut()
                .reconcile(listed, listing_started_ms),
            Err(error) => shared
                .engine
                .borrow()
                .log
                .warn(format_args!("session listing gave no information: {error}")),
        }
        let listed_at = Instant::now();
        let mut cache_due = listed_at + cache_interval;
        loop {
            let periodic = shared.engine.borrow().has_live_sessions();
            tokio::select! {
                () = sleep_until(cache_due) => {
                    cache.refresh(&shared).await?;
                    cache_due += cache_interval;
                }
                () = sleep_until(listed_at + interval), if periodic => break,
                changed = changes.changed() => {
                    if changed.is_err() {
                        return Ok(());
                    }
                    if shared.engine.borrow_mut().take_reconcile_wanted() {
                        sleep_until(listed_at + RECONCILE_SPACING).await;
                        break;
                    }
                }
            }
        }
    }
}

struct CacheWatch {
    path: Option<PathBuf>,
    modified: Option<SystemTime>,
    warned: bool,
}

impl CacheWatch {
    async fn refresh(&mut self, shared: &Shared) -> io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        let modified = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .ok();
        if modified.is_some() && modified == self.modified {
            return Ok(());
        }
        self.modified = modified;
        match spawn_blocking(move || account::read_cache(&path))
            .await
            .map_err(io::Error::other)?
        {
            Some(usage) => {
                self.warned = false;
                shared.engine.borrow_mut().offer_account_usage(usage);
            }
            None if !self.warned => {
                self.warned = true;
                shared
                    .engine
                    .borrow()
                    .log
                    .warn("the Claude Code config has no readable cachedUsageUtilization");
            }
            None => {}
        }
        Ok(())
    }
}

async fn expire_snoozes(shared: Rc<Shared>) -> io::Result<()> {
    let mut changes = shared.engine.borrow().watch_changes();
    loop {
        let expiry = shared.engine.borrow().next_snooze_expiry();
        let until_expiry =
            Duration::from_millis(expiry.unwrap_or(u64::MAX).saturating_sub(now_ms()));
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return Ok(());
                }
            }
            () = sleep(until_expiry), if expiry.is_some() => {
                shared.engine.borrow_mut().expire_snoozes(now_ms());
            }
        }
    }
}

async fn read_usage(shared: Rc<Shared>) -> io::Result<()> {
    let mut changes = shared.engine.borrow().watch_changes();
    let mut readers: HashMap<String, TranscriptReader> = HashMap::new();
    let mut last_read: HashMap<String, Instant> = HashMap::new();
    let mut due: BTreeMap<String, (Instant, String)> = BTreeMap::new();
    loop {
        let wanted = shared.engine.borrow_mut().take_usage_wanted();
        for (session_id, transcript) in wanted {
            let earliest = last_read
                .get(&session_id)
                .map_or_else(Instant::now, |read_at| *read_at + USAGE_READ_INTERVAL);
            due.entry(session_id)
                .and_modify(|entry| entry.1 = transcript.clone())
                .or_insert((earliest, transcript));
        }
        let next = due.values().map(|(at, _)| *at).min();
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return Ok(());
                }
            }
            () = sleep_until(next.unwrap_or_else(Instant::now)), if next.is_some() => {
                let now = Instant::now();
                let ready: Vec<String> = due
                    .iter()
                    .filter(|(_, (at, _))| *at <= now)
                    .map(|(session_id, _)| session_id.clone())
                    .collect();
                for session_id in ready {
                    let Some((_, transcript)) = due.remove(&session_id) else {
                        continue;
                    };
                    last_read.insert(session_id.clone(), Instant::now());
                    let mut reader = readers
                        .remove(&session_id)
                        .filter(|reader| reader.transcript() == Path::new(&transcript))
                        .unwrap_or_else(|| TranscriptReader::new(&session_id, Path::new(&transcript)));
                    let (reader, read) = spawn_blocking(move || {
                        let read = reader.read(now_ms()).map(|usage| (usage, reader.take_titles()));
                        (reader, read)
                    })
                    .await
                    .map_err(io::Error::other)?;
                    readers.insert(session_id.clone(), reader);
                    let mut engine = shared.engine.borrow_mut();
                    match read {
                        Ok((usage, titles)) => {
                            engine.set_usage(&session_id, usage);
                            engine.set_titles(&session_id, titles);
                        }
                        Err(error) => engine.log.warn(format_args!(
                            "usage of session {session_id} gave no information: {error}"
                        )),
                    }
                }
                let engine = shared.engine.borrow();
                readers.retain(|session_id, _| engine.has_session(session_id));
                last_read.retain(|session_id, _| engine.has_session(session_id));
            }
        }
    }
}

async fn read_plans(shared: Rc<Shared>) -> io::Result<()> {
    let mut changes = shared.engine.borrow().watch_changes();
    while changes.changed().await.is_ok() {
        let wanted = shared.engine.borrow_mut().take_plans_wanted();
        for id in wanted {
            tokio::task::spawn_local(fetch_plan(shared.clone(), id));
        }
    }
    Ok(())
}

async fn find_transcripts(shared: Rc<Shared>, projects: Option<PathBuf>) -> io::Result<()> {
    let mut changes = shared.engine.borrow().watch_changes();
    loop {
        let wanted = shared.engine.borrow_mut().take_transcripts_wanted();
        if let Some(projects) = projects.clone().filter(|_| !wanted.is_empty()) {
            let found = spawn_blocking(move || {
                wanted
                    .into_iter()
                    .filter_map(|session_id| {
                        let transcript = find_transcript(&projects, &session_id)?;
                        Some((session_id, transcript.to_string_lossy().into_owned()))
                    })
                    .collect::<Vec<_>>()
            })
            .await
            .map_err(io::Error::other)?;
            let mut engine = shared.engine.borrow_mut();
            for (session_id, transcript) in found {
                engine.adopt_transcript(&session_id, transcript);
            }
        }
        if changes.changed().await.is_err() {
            return Ok(());
        }
    }
}

async fn fetch_plan(shared: Rc<Shared>, id: u64) {
    for delay in PLAN_RETRY_DELAYS {
        sleep(delay).await;
        let Some((transcript, tool_use_id)) = shared.engine.borrow().plan_source(id) else {
            return;
        };
        let found =
            spawn_blocking(move || find_plan(Path::new(&transcript), tool_use_id.as_deref()))
                .await
                .map_err(io::Error::other)
                .and_then(|found| found);
        match found {
            Ok(Some(plan)) => return shared.engine.borrow_mut().set_plan(id, plan),
            Ok(None) => {}
            Err(error) => shared.engine.borrow().log.debug(format_args!(
                "plan of decision {id} not readable yet: {error}"
            )),
        }
    }
    shared.engine.borrow().log.info(format_args!(
        "no plan text found in the transcript for decision {id}"
    ));
}

async fn check_stalls(shared: Rc<Shared>) -> io::Result<()> {
    loop {
        sleep(STALL_CHECK_INTERVAL).await;
        shared.engine.borrow_mut().check_stalls(now_ms());
    }
}

async fn persist(shared: Rc<Shared>, home: PathBuf) -> io::Result<()> {
    let mut changes = shared.engine.borrow().watch_changes();
    let path = home.join("state.json");
    while changes.changed().await.is_ok() {
        sleep(PERSIST_DEBOUNCE).await;
        changes.borrow_and_update();
        let state = shared.engine.borrow().persisted_state()?;
        if let Err(error) = write_atomic(&path, &state) {
            shared.engine.borrow().log.warn(format_args!(
                "cannot write state.json, trying again on the next change: {error}"
            ));
        }
    }
    Ok(())
}
