use crate::shell::{self, Action};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};
use tauri::AppHandle;

pub struct Wakes {
    sender: Sender<Instant>,
}

fn take_due(pending: &mut BinaryHeap<Reverse<Instant>>, now: Instant) -> bool {
    let mut due = false;
    while pending.peek().is_some_and(|Reverse(at)| *at <= now) {
        pending.pop();
        due = true;
    }
    due
}

impl Wakes {
    pub fn start(app: AppHandle) -> Wakes {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut pending: BinaryHeap<Reverse<Instant>> = BinaryHeap::new();
            loop {
                let received = match pending.peek() {
                    Some(Reverse(at)) => {
                        receiver.recv_timeout(at.saturating_duration_since(Instant::now()))
                    }
                    None => receiver.recv().map_err(|_| RecvTimeoutError::Disconnected),
                };
                match received {
                    Ok(at) => pending.push(Reverse(at)),
                    Err(RecvTimeoutError::Timeout) => {
                        if take_due(&mut pending, Instant::now()) {
                            shell::post(&app, Action::Recheck);
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        });
        Wakes { sender }
    }

    pub fn after(&self, delay: Duration) {
        if let Err(error) = self.sender.send(Instant::now() + delay) {
            log_error!("cannot schedule a recheck: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_wakes_that_have_come_due_are_taken_and_the_later_ones_stay() {
        let start = Instant::now();
        let at = |seconds| start + Duration::from_secs(seconds);
        let mut pending: BinaryHeap<_> = [at(9), at(2), at(5)].into_iter().map(Reverse).collect();
        assert!(!take_due(&mut pending, at(1)));
        assert_eq!(pending.len(), 3);
        assert!(take_due(&mut pending, at(5)));
        assert_eq!(pending.len(), 1);
        assert_eq!(pending.peek(), Some(&Reverse(at(9))));
        assert!(!take_due(&mut pending, at(8)));
    }
}
