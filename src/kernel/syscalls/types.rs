use core::ptr::null_mut;
use crate::*;

pub const UNIX_SOCK_BUFF_DEFAULT: usize = 4096;
pub const UNIX_SOCK_POLL_EXTRA: usize = 128;

#[repr(C)]
pub struct UnixSocketPair {
    pub LOCK_PAIR: Spinlock,
    pub client_fds: usize,
    pub server_fds: usize,
    pub established: bool,

    pub client_buff: CircularBuffer,
    pub server_buff: CircularBuffer,
    pub client_buff_size: usize,
    pub server_buff_size: usize,

    pub filename: *mut u8,
}

#[repr(C)]
pub struct UnixSocket {
    pub LOCK_SOCK: Spinlock,
    pub times_opened: usize,

    pub bind_addr: *mut u8,

    pub conn_max: usize,
    pub conn_curr: usize,
    pub backlog: *mut *mut UnixSocketPair,
    pub accept_would_block: bool,

    pub pair: *mut UnixSocketPair,
}

/* global list */
extern "C" {
    pub static mut dsUnixSocket: LinkedList;
    pub static mut LOCK_LL_UNIX_SOCKET: Spinlock;
}
