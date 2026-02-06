use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;

// -------------------- Simulated Kernel Structures --------------------

#[derive(Debug)]
struct Task {
    id: usize,
    state: TaskState,
    forceful_wakeup_time: Option<u64>, // milliseconds
    // Simulated signal info
    info_signals: Arc<Mutex<InfoSignals>>,
}

#[derive(Debug, PartialEq)]
enum TaskState {
    Ready,
    Blocked,
}

#[derive(Debug)]
struct Itimer {
    at: u64,
    reset: u64,
}

#[derive(Debug)]
struct InfoSignals {
    itimer_real: Itimer,
}

#[derive(Debug, Clone)]
struct Timeval {
    tv_sec: u64,
    tv_usec: u64,
}

#[derive(Debug, Clone)]
struct Timespec {
    tv_sec: u64,
    tv_nsec: u64,
}

// -------------------- Globals --------------------

static mut TIMER_BOOT_UNIX: u64 = 1_700_000_000_000; // example epoch ms
static mut TIMER_TICKS: u64 = 0;

// -------------------- Helpers --------------------

fn hand_control() {
    thread::yield_now();
}

fn signals_pending_quick(_task: &Task) -> bool {
    false
}

fn div_round_up(a: u64, b: u64) -> u64 {
    (a + b - 1) / b
}

// -------------------- Time Conversions --------------------

fn ms_to_timeval(ms: u64) -> Timeval {
    Timeval {
        tv_sec: ms / 1000,
        tv_usec: (ms % 1000) * 1000,
    }
}

fn timeval_to_ms(tv: &Timeval) -> u64 {
    tv.tv_sec * 1000 + div_round_up(tv.tv_usec, 1000)
}

// -------------------- Syscalls --------------------

fn syscall_nanosleep(current_task: &mut Task, duration: &Timespec) -> Result<(), i32> {
    if duration.tv_sec as i64  < 0 {
        return Err(libc::EINVAL);
    }

    let ms = duration.tv_sec * 1000 + duration.tv_nsec / 1_000_000;

    unsafe {
        current_task.forceful_wakeup_time = Some(TIMER_TICKS + ms);
    }
    current_task.state = TaskState::Blocked;

    while current_task.forceful_wakeup_time.unwrap_or(0) > unsafe { TIMER_TICKS } {
        hand_control();
    }

    current_task.forceful_wakeup_time = None;

    if signals_pending_quick(current_task) {
        return Err(libc::EINTR);
    }

    Ok(())
}

fn syscall_setitimer(current_task: &mut Task, value: Option<&Itimer>, old: Option<&mut Itimer>) -> Result<(), i32> {
    let mut signals = current_task.info_signals.lock().unwrap();

    if let Some(old_timer) = old {
        let rt_at = signals.itimer_real.at;
        let rt_reset = signals.itimer_real.reset;
        old_timer.at = rt_at.saturating_sub(unsafe { TIMER_TICKS });
        old_timer.reset = rt_reset;
    }

    if let Some(val) = value {
        signals.itimer_real.at = if val.at > 0 { unsafe { TIMER_TICKS } + val.at } else { 0 };
        signals.itimer_real.reset = val.reset;
    }

    Ok(())
}

fn syscall_clock_gettime(which: i32, spec: &mut Timespec) -> Result<(), i32> {
    unsafe {
        let time = TIMER_BOOT_UNIX + TIMER_TICKS;
        match which {
            0 | 4 | 6 | 1 => { // CLOCK_REALTIME, MONOTONIC etc.
                spec.tv_sec = time / 1000;
                spec.tv_nsec = (time % 1000) * 1_000_000;
                Ok(())
            }
            7 => { // CLOCK_BOOTTIME
                spec.tv_sec = TIMER_BOOT_UNIX / 1000;
                spec.tv_nsec = (TIMER_BOOT_UNIX % 1000) * 1_000_000;
                Ok(())
            }
            _ => Err(libc::EINVAL),
        }
    }
}

fn syscall_clock_getres(_which: i32, spec: &mut Timespec) -> Result<(), i32> {
    spec.tv_sec = 0;
    spec.tv_nsec = 1_000_000; // 1ms resolution
    Ok(())
}

// -------------------- Example Registration --------------------

fn syscalls_reg_clock() {
    // Example placeholder for syscall registration
    println!("Syscalls registered: nanosleep, clock_gettime, clock_getres, setitimer");
}
