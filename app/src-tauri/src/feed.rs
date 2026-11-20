use beacon_core::{
    AccountSources, ClaudeAgentsLister, ClaudeUsageProber, Config, ControlClient, Core, CoreHandle,
    CoreOptions, Reply, Request, Snapshot, claude_json_path,
};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const RETRY: Duration = Duration::from_secs(5);

pub struct Feed {
    pub home: PathBuf,
    hosted: Mutex<Option<CoreHandle>>,
    quitting: AtomicBool,
}

impl Feed {
    pub fn new(home: PathBuf) -> Arc<Feed> {
        Arc::new(Feed {
            home,
            hosted: Mutex::new(None),
            quitting: AtomicBool::new(false),
        })
    }

    pub fn start(
        self: &Arc<Feed>,
        publish: impl Fn(Option<Snapshot>) + Send + 'static,
    ) -> io::Result<()> {
        let feed = self.clone();
        thread::Builder::new()
            .name("beacon-feed".to_string())
            .spawn(move || feed.run(publish))
            .map(drop)
    }

    pub fn shutdown(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        if let Some(core) = self.hosted.lock().unwrap().take()
            && let Err(error) = core.shutdown()
        {
            eprintln!("the Beacon core did not stop cleanly: {error}");
        }
    }

    fn quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }

    fn run(&self, publish: impl Fn(Option<Snapshot>)) {
        while !self.quitting() {
            self.host();
            match ControlClient::connect(&self.home).and_then(ControlClient::subscribe) {
                Ok(subscription) => {
                    let mut closed_by_core = true;
                    for reply in subscription {
                        match reply {
                            Ok(Reply {
                                snapshot: Some(snapshot),
                                ..
                            }) => publish(Some(snapshot)),
                            Ok(_) => {}
                            Err(error) => {
                                eprintln!("the Beacon subscription failed: {error}");
                                closed_by_core = false;
                                break;
                            }
                        }
                    }
                    if closed_by_core {
                        self.reap();
                    }
                }
                Err(error) => eprintln!("cannot connect to the Beacon core: {error}"),
            }
            if !self.quitting() {
                publish(None);
                thread::sleep(RETRY);
            }
        }
    }

    fn host(&self) {
        let mut hosted = self.hosted.lock().unwrap();
        if hosted.is_some() || self.quitting() {
            return;
        }
        let started = Config::load(&self.home).and_then(|config| {
            Core::start_with(
                CoreOptions {
                    home: self.home.clone(),
                    lister: Box::new(ClaudeAgentsLister),
                    config,
                },
                AccountSources {
                    claude_json: claude_json_path(),
                    prober: Some(Box::new(ClaudeUsageProber)),
                },
            )
        });
        match started {
            Ok(core) => *hosted = Some(core),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => eprintln!("cannot host the Beacon core: {error}"),
        }
    }

    fn reap(&self) {
        let stopped = self.hosted.lock().unwrap().take();
        if let Some(core) = stopped
            && let Err(error) = core.wait()
        {
            eprintln!("the hosted Beacon core stopped: {error}");
        }
    }
}

pub fn request(home: &Path, request: &Request) -> Result<Reply, String> {
    let reply = ControlClient::connect(home)
        .and_then(|mut client| client.request(request))
        .map_err(|error| error.to_string())?;
    if reply.ok {
        Ok(reply)
    } else {
        Err(reply
            .error
            .unwrap_or_else(|| "the Beacon core refused the request".to_string()))
    }
}

pub fn send(home: PathBuf, message: Request) {
    thread::spawn(move || {
        if let Err(error) = request(&home, &message) {
            eprintln!("the Beacon core did not take {message:?}: {error}");
        }
    });
}
