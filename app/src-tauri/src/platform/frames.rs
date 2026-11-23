use crate::motion::frame_interval;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::thread::{self, Thread};
use std::time::Instant;
use windows_sys::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, INFINITE, SetWaitableTimer,
    TIMER_ALL_ACCESS, WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_FRAME: u32 = WM_APP + 3;

static RUNNING: AtomicBool = AtomicBool::new(false);
static POSTED: AtomicBool = AtomicBool::new(false);
static RATE: AtomicU32 = AtomicU32::new(60);
static TARGET: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static PACER: OnceLock<Thread> = OnceLock::new();

pub fn run(window: *mut c_void, refresh_rate: u32) {
    TARGET.store(window, Ordering::Relaxed);
    RATE.store(refresh_rate, Ordering::Relaxed);
    if !RUNNING.swap(true, Ordering::AcqRel) {
        PACER
            .get_or_init(|| thread::spawn(pace).thread().clone())
            .unpark();
    }
}

pub fn stop() {
    RUNNING.store(false, Ordering::Release);
}

pub fn received() {
    POSTED.store(false, Ordering::Release);
}

fn pace() {
    let timer = unsafe {
        CreateWaitableTimerExW(
            null(),
            null(),
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
            TIMER_ALL_ACCESS,
        )
    };
    if timer.is_null() {
        log_error!(
            "cannot create the frame timer: {}",
            std::io::Error::last_os_error()
        );
        return;
    }
    loop {
        while !RUNNING.load(Ordering::Acquire) {
            thread::park();
        }
        let mut next = Instant::now();
        while RUNNING.load(Ordering::Acquire) {
            let interval = frame_interval(RATE.load(Ordering::Relaxed));
            next += interval;
            let now = Instant::now();
            if next <= now {
                next = now + interval;
            }
            let due = -(((next - now).as_nanos() / 100) as i64);
            unsafe {
                SetWaitableTimer(timer, &due, 0, None, null(), 0);
                WaitForSingleObject(timer, INFINITE);
            }
            if RUNNING.load(Ordering::Acquire) && !POSTED.swap(true, Ordering::AcqRel) {
                unsafe { PostMessageW(TARGET.load(Ordering::Relaxed), WM_FRAME, 0, 0) };
            }
        }
    }
}
