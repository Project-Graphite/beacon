use crate::config::Config;
use crate::control::{Reply, Request};
use crate::engine::{Engine, HookOutcome, ProcessExit};
use crate::lister::SessionLister;
use crate::log::{Logger, now_ms};
use crate::platform::{self, InstanceLock, PIPE_PREFIX, PipeSecurity};
use beacon_protocol::{
    Channel, ENDPOINT_FILE, Endpoint, HookReply, MAX_FRAME_LEN, TOKEN_LEN, channel_for, frame_len,
    tokens_match,
};
use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio::time::{Instant, sleep, sleep_until, timeout};

const LISTENERS: usize = 2;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const PERSIST_DEBOUNCE: Duration = Duration::from_millis(500);
const SUBSCRIBE_INTERVAL: Duration = Duration::from_millis(100);
const STALL_CHECK_INTERVAL: Duration = Duration::from_secs(30);

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
        let shutdown = Arc::new(watch::channel(false).0);
        let (ready, started) = std::sync::mpsc::channel();
        let thread_shutdown = shutdown.clone();
        let thread = std::thread::Builder::new()
            .name("beacon-core".to_string())
            .spawn(move || run(options, thread_shutdown, ready))?;
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
    token: [u8; TOKEN_LEN],
    pipe_name: String,
    security: PipeSecurity,
    shutdown: Arc<watch::Sender<bool>>,
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
    shutdown: Arc<watch::Sender<bool>>,
    ready: std::sync::mpsc::Sender<io::Result<Endpoint>>,
) -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let started = {
        let _context = runtime.enter();
        start(&options, shutdown.clone())
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
        tasks.spawn_local(reconcile(shared.clone(), options.lister, options.config.reconcile_seconds));
        tasks.spawn_local(check_stalls(shared.clone()));
        tasks.spawn_local(persist(shared.clone(), options.home.clone()));
        let mut shutdown_requested = shutdown.subscribe();
        tokio::select! {
            _ = shutdown_requested.wait_for(|requested| *requested) => Ok(()),
            Some(finished) = tasks.join_next() => finished.map_err(io::Error::other).and_then(|result| result),
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

fn start(options: &CoreOptions, shutdown: Arc<watch::Sender<bool>>) -> io::Result<Started> {
    std::fs::create_dir_all(&options.home)?;
    let lock = InstanceLock::acquire(&options.home)?;
    let log = Logger::open(
        &options.home.join("logs").join("core.log"),
        options.config.log_level,
    )?;
    let sessions = match std::fs::read(options.home.join("state.json")) {
        Ok(bytes) => Engine::load_sessions(&bytes)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
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
    let (engine, exits) = Engine::new(options.config.clone(), log, sessions);
    let mut token = [0u8; TOKEN_LEN];
    token.copy_from_slice(endpoint.token.as_bytes());
    Ok(Started {
        shared: Rc::new(Shared {
            engine: RefCell::new(engine),
            token,
            pipe_name: endpoint.pipe_name.clone(),
            security,
            shutdown,
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

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
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
        tokio::task::spawn_local(async move {
            if let Err(error) = serve(&connection, client).await {
                connection
                    .engine
                    .borrow()
                    .log
                    .debug(format_args!("connection dropped: {error}"));
            }
        });
    }
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
            let mut probe = [0u8; 1];
            let waited = tokio::select! {
                reply = decided => Some(reply),
                _ = pipe.read(&mut probe) => None,
            };
            match waited {
                Some(Ok(reply)) => send_reply(&mut pipe, reply).await,
                Some(Err(_)) => Ok(()),
                None => {
                    shared.engine.borrow_mut().helper_gone(id);
                    Ok(())
                }
            }
        }
    }
}

async fn send_reply(pipe: &mut NamedPipeServer, reply: HookReply) -> io::Result<()> {
    pipe.write_all(&reply.encode()?).await?;
    let mut probe = [0u8; 1];
    let _ = timeout(CLOSE_TIMEOUT, pipe.read(&mut probe)).await;
    Ok(())
}

async fn serve_control(shared: &Shared, pipe: NamedPipeServer) -> io::Result<()> {
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
        let snapshot = shared.engine.borrow().snapshot();
        write_reply(writer, &Reply::with_snapshot(snapshot)).await?;
        let next_allowed = Instant::now() + SUBSCRIBE_INTERVAL;
        if changes.changed().await.is_err() {
            return Ok(());
        }
        sleep_until(next_allowed).await;
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

async fn reconcile(
    shared: Rc<Shared>,
    lister: Box<dyn SessionLister>,
    reconcile_seconds: u64,
) -> io::Result<()> {
    let interval = Duration::from_secs(reconcile_seconds.max(1));
    loop {
        match lister.list().await {
            Ok(listed) => shared.engine.borrow_mut().reconcile(listed),
            Err(error) => shared
                .engine
                .borrow()
                .log
                .warn(format_args!("session listing gave no information: {error}")),
        }
        loop {
            sleep(interval).await;
            if shared.engine.borrow().has_live_sessions() {
                break;
            }
        }
    }
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
        write_atomic(&path, &state)?;
    }
    Ok(())
}
