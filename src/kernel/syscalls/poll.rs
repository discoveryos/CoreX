use core::ptr;
use core::mem;
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::alloc::{alloc_zeroed, dealloc, Layout};
use crate::task::*;
use crate::fs::*;
use crate::timer::*;
use crate::spinlock::*;
use crate::linked_list::*;
use crate::syscalls::*;
use crate::socket::*;

// Poll Instance helpers
pub struct PollItem {
    pub key: u64,
    pub epoll_events: i32,
    pub next: Option<Box<PollItem>>,
}

pub struct TaskListener {
    pub task: *mut Task,
    pub next: Option<Box<TaskListener>>,
}

pub struct PollInstance {
    pub listeners: LinkedList<TaskListener>,
    pub items: LinkedList<PollItem>,
    pub listening: bool,
}

impl PollInstance {
    pub fn new() -> Self {
        Self {
            listeners: LinkedList::new(),
            items: LinkedList::new(),
            listening: false,
        }
    }

    pub fn add_item(&mut self, key: u64, events: i32) {
        let item = Box::new(PollItem {
            key,
            epoll_events: events,
            next: None,
        });
        self.items.push_front(item);
    }

    pub fn remove_item(&mut self, key: u64) -> Option<Box<PollItem>> {
        self.items.remove(|item| item.key == key)
    }

    pub fn lookup_item(&self, key: u64) -> Option<&PollItem> {
        self.items.find(|item| item.key == key)
    }
}

pub fn poll_instance_ring(instance: &mut PollInstance, key: u64) {
    if let Some(item) = instance.lookup_item(key) {
        poll_instance_ring_inner(instance, false);
    }
}

pub fn poll_instance_ring_inner(instance: &mut PollInstance, involuntary: bool) {
    let mut current = instance.listeners.first_mut();
    while let Some(listener) = current {
        unsafe {
            if !involuntary && (*listener.task).state == TaskState::Ready {
                assert!((*listener.task).extras & EXTRAS_INVOLUNTARY_WAKEUP != 0);
                return;
            }
            (*listener.task).forceful_wakeup_time = 0;
            (*listener.task).state = TaskState::Ready;
        }
        current = listener.next.take();
    }
    instance.listeners.clear();
    instance.listening = false;
}

// Epoll
pub struct EpollWatch {
    pub fd: *mut OpenFile,
    pub watch_events: u32,
    pub user_data: u64,
}

pub struct Epoll {
    pub instance: PollInstance,
    pub times_opened: usize,
    pub watches: LinkedList<EpollWatch>,
}

impl Epoll {
    pub fn new() -> Self {
        Self {
            instance: PollInstance::new(),
            times_opened: 1,
            watches: LinkedList::new(),
        }
    }
}

// Epoll helper
pub fn epoll_to_poll(epoll_events: u32) -> u32 {
    let mut poll_events = 0;
    if epoll_events & EPOLLIN != 0 { poll_events |= POLLIN; }
    if epoll_events & EPOLLOUT != 0 { poll_events |= POLLOUT; }
    if epoll_events & EPOLLPRI != 0 { poll_events |= POLLPRI; }
    if epoll_events & EPOLLERR != 0 { poll_events |= POLLERR; }
    if epoll_events & EPOLLHUP != 0 { poll_events |= POLLHUP; }
    poll_events
}

pub fn poll_to_epoll(poll_events: u32) -> u32 {
    let mut epoll_events = 0;
    if poll_events & POLLIN != 0 { epoll_events |= EPOLLIN; }
    if poll_events & POLLOUT != 0 { epoll_events |= EPOLLOUT; }
    if poll_events & POLLPRI != 0 { epoll_events |= EPOLLPRI; }
    if poll_events & POLLERR != 0 { epoll_events |= EPOLLERR; }
    if poll_events & POLLHUP != 0 { epoll_events |= EPOLLHUP; }
    epoll_events
}

// Independent Poll Await
pub fn poll_independent_await(fd: &mut OpenFile, events: i32) {
    let mut instance = PollInstance::new();
    unsafe {
        let key = ((*fd.handlers).report_key)(fd);
        if ((*fd.handlers).internal_poll)(fd, events) != 0 {
            return;
        }
        instance.add_item(key, events);
    }
    // simulate blocking wait
    poll_instance_wait(&mut instance, 0);
}

// Poll wait
pub fn poll_instance_wait(instance: &mut PollInstance, expiry: u64) {
    unsafe {
        let listener = TaskListener {
            task: current_task(),
            next: None,
        };
        instance.listeners.push_front(listener);
        instance.listening = true;
        if expiry != 0 {
            (*current_task()).forceful_wakeup_time = expiry;
        }
        hand_control();
    }
}
