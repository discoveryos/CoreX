use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// -------------------- Simulated Kernel Structures --------------------

#[derive(Debug)]
struct Task {
    id: usize,
    state: TaskState,
}

#[derive(Debug, PartialEq)]
enum TaskState {
    Ready,
    Waiting,
}

struct OpenFile {
    flags: u32,
    dir: Arc<Mutex<EventFd>>,
}

const O_NONBLOCK: u32 = 0x800;
const EPOLLIN: u32 = 0x001;
const EPOLLOUT: u32 = 0x004;

fn signals_pending_quick(_task: &Task) -> bool {
    false // simulate
}

fn hand_control() {
    thread::yield_now(); // simulate scheduler handoff
}

// -------------------- EventFd --------------------

struct EventFd {
    counter: u64,
    utilized_by: u32,
}

impl EventFd {
    fn new(init: u64) -> Self {
        EventFd {
            counter: init,
            utilized_by: 1,
        }
    }

    fn read(&mut self, task: &Task, nonblock: bool) -> Result<u64, i32> {
        while self.counter == 0 {
            if signals_pending_quick(task) {
                return Err(libc::EINTR);
            }
            if nonblock {
                return Err(libc::EWOULDBLOCK);
            }
            hand_control();
        }
        let val = self.counter;
        self.counter = 0;
        Ok(val)
    }

    fn write(&mut self, task: &Task, value: u64, nonblock: bool) -> Result<(), i32> {
        if value == u64::MAX {
            return Err(libc::EINVAL);
        }

        while value > u64::MAX - self.counter {
            if signals_pending_quick(task) {
                return Err(libc::EINTR);
            }
            if nonblock {
                return Err(libc::EWOULDBLOCK);
            }
            hand_control();
        }

        self.counter += value;
        Ok(())
    }

    fn poll(&self, events: u32) -> u32 {
        let mut revents = 0;
        if (events & EPOLLIN) != 0 && self.counter > 0 {
            revents |= EPOLLIN;
        }
        if (events & EPOLLOUT) != 0 && self.counter < u64::MAX {
            revents |= EPOLLOUT;
        }
        revents
    }
}

// -------------------- Handlers --------------------

fn eventfd_open(init_value: u64, flags: u32) -> Arc<Mutex<EventFd>> {
    Arc::new(Mutex::new(EventFd::new(init_value)))
}

fn eventfd_read(fd: &Arc<Mutex<EventFd>>, task: &Task, nonblock: bool) -> Result<u64, i32> {
    let mut fd_guard = fd.lock().unwrap();
    fd_guard.read(task, nonblock)
}

fn eventfd_write(fd: &Arc<Mutex<EventFd>>, task: &Task, value: u64, nonblock: bool) -> Result<(), i32> {
    let mut fd_guard = fd.lock().unwrap();
    fd_guard.write(task, value, nonblock)
}

fn eventfd_poll(fd: &Arc<Mutex<EventFd>>, events: u32) -> u32 {
    let fd_guard = fd.lock().unwrap();
    fd_guard.poll(events)
}

fn eventfd_duplicate(fd: &Arc<Mutex<EventFd>>) -> Arc<Mutex<EventFd>> {
    let mut fd_guard = fd.lock().unwrap();
    fd_guard.utilized_by += 1;
    drop(fd_guard);
    Arc::clone(fd)
}

fn eventfd_close(fd: Arc<Mutex<EventFd>>) {
    let mut fd_guard = fd.lock().unwrap();
    if fd_guard.utilized_by > 1 {
        fd_guard.utilized_by -= 1;
    }
    // otherwise drop fd naturally by Rust ownership
}
