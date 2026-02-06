use crate::task::*;
use crate::fs::*;
use crate::poll::*;
use crate::system::*;
use crate::timer::*;

// ==========================
// Syscall: epoll_create
// ==========================
pub fn syscall_epoll_create(_size: i32) -> Result<usize, i32> {
    Ok(epoll_create1(0))
}

// ==========================
// Syscall: epoll_create1
// ==========================
pub fn syscall_epoll_create1(flags: i32) -> Result<usize, i32> {
    Ok(epoll_create1(flags))
}

// ==========================
// Syscall: epoll_ctl
// ==========================
pub fn syscall_epoll_ctl(epfd: i32, op: i32, fd: i32, event: &mut epoll_event) -> Result<usize, i32> {
    let epoll_fd = fs_user_get_node(current_task(), epfd).ok_or(EBADF)?;
    Ok(epoll_ctl(epoll_fd, op, fd, event))
}

// ==========================
// Syscall: epoll_wait
// ==========================
pub fn syscall_epoll_wait(epfd: i32, events: &mut [epoll_event], maxevents: i32, timeout: i32) -> Result<usize, i32> {
    let epoll_fd = fs_user_get_node(current_task(), epfd).ok_or(EBADF)?;
    Ok(epoll_wait(epoll_fd, events, maxevents, timeout))
}

// ==========================
// Syscall: epoll_pwait
// ==========================
pub fn syscall_epoll_pwait(
    epfd: i32,
    events: &mut [epoll_event],
    maxevents: i32,
    timeout: i32,
    sigmask: Option<&sigset_t>,
    sigsetsize: usize,
) -> Result<usize, i32> {
    let epoll_fd = fs_user_get_node(current_task(), epfd).ok_or(EBADF)?;
    Ok(epoll_pwait(epoll_fd, events, maxevents, timeout, sigmask, sigsetsize))
}

// ==========================
// Register poll syscalls
// ==========================
pub fn syscalls_reg_poll() {
    register_syscall(SYSCALL_EPOLL_CREATE, syscall_epoll_create as usize);
    register_syscall(SYSCALL_EPOLL_CREATE1, syscall_epoll_create1 as usize);
    register_syscall(SYSCALL_EPOLL_CTL, syscall_epoll_ctl as usize);
    register_syscall(SYSCALL_EPOLL_WAIT, syscall_epoll_wait as usize);
    register_syscall(SYSCALL_EPOLL_PWAIT, syscall_epoll_pwait as usize);
}
