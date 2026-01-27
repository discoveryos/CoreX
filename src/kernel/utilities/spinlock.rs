#![allow(dead_code)]

use core::sync::atomic::{AtomicBool, Ordering};
use core::hint::spin_loop;

//
// External kernel hooks (provided elsewhere)
//
extern "C" {
    fn handControl();
    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    static timerTicks: u64;
}

//
// Spinlock
//
pub struct Spinlock {
    flag: AtomicBool,
}

impl Spinlock {
    pub const fn new() -> Self {
        Self {
            flag: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn acquire(&self) {
        while self
            .flag
            .swap(true, Ordering::Acquire)
        {
            unsafe { handControl() };
            spin_loop();
        }
    }

    #[inline]
    pub fn release(&self) {
        self.flag.store(false, Ordering::Release);
    }
}

//
// Counter spinlock (RW-like)
//
pub struct SpinlockCnt {
    pub lock: Spinlock,
    pub cnt: i32, // >0 readers, 0 free, -1 writer
}

impl SpinlockCnt {
    pub const fn new() -> Self {
        Self {
            lock: Spinlock::new(),
            cnt: 0,
        }
    }

    pub fn read_acquire(&mut self) {
        loop {
            self.lock.acquire();
            if self.cnt > -1 {
                self.cnt += 1;
                self.lock.release();
                return;
            }
            self.lock.release();
            unsafe { handControl() };
            spin_loop();
        }
    }

    pub fn read_release(&mut self) {
        self.lock.acquire();

        if self.cnt < 0 {
            unsafe {
                debugf(b"[spinlock] Something very bad is going on...\n\0".as_ptr());
                panic();
            }
        }

        self.cnt -= 1;
        self.lock.release();
    }

    pub fn write_acquire(&mut self) {
        loop {
            self.lock.acquire();
            if self.cnt == 0 {
                self.cnt = -1;
                self.lock.release();
                return;
            }
            self.lock.release();
            unsafe { handControl() };
            spin_loop();
        }
    }

    pub fn write_release(&mut self) {
        self.lock.acquire();

        if self.cnt != -1 {
            unsafe {
                debugf(b"[spinlock] Something very bad is going on...\n\0".as_ptr());
                panic();
            }
        }

        self.cnt = 0;
        self.lock.release();
    }
}

//
// Semaphore
//
pub struct Semaphore {
    pub lock: Spinlock,
    pub cnt: i32,
}

impl Semaphore {
    pub const fn new(initial: i32) -> Self {
        Self {
            lock: Spinlock::new(),
            cnt: initial,
        }
    }

    pub fn wait(&mut self, timeout: u32) -> bool {
        let start = unsafe { timerTicks };
        let mut ret = false;

        loop {
            if timeout > 0 {
                let now = unsafe { timerTicks };
                if now > start + timeout as u64 {
                    break;
                }
            }

            self.lock.acquire();
            if self.cnt > 0 {
                self.cnt -= 1;
                ret = true;
                self.lock.release();
                break;
            }
            self.lock.release();

            unsafe { handControl() };
            spin_loop();
        }

        ret
    }

    pub fn post(&mut self) {
        self.lock.acquire();
        self.cnt += 1;
        self.lock.release();
    }
}
