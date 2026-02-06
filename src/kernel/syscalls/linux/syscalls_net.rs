use crate::task::*;
use crate::fs::*;
use crate::unix_socket::*;
use crate::socket::*;
use crate::system::*;
use crate::timer::*;
use crate::lwip::*; // bindings to lwIP

use core::ptr::null_mut;

// ==========================
// sockaddr conversion
// ==========================
pub fn sockaddr_linux_to_lwip(addr: &mut sockaddr_linux, addrlen: u32) -> u16 {
    let initial_family = addr.sa_family;
    addr.sa_len = addrlen;
    addr.sa_family = AF_INET as u16; // convert to lwIP style
    initial_family
}

pub fn sockaddr_lwip_to_linux(addr: &mut sockaddr_linux, initial_family: u16) {
    addr.sa_family = initial_family;
}

// ==========================
// Syscall: socket
// ==========================
pub fn syscall_socket(task: &mut Task, mut family: i32, mut ty: i32, protocol: i32) -> Result<usize, i32> {
    match family {
        AF_UNIX => Ok(unix_socket_open(task, ty, protocol)?),

        AF_INET6 => {
            let socket_fd = fs_user_open(task, "/dev/null", O_RDWR, 0)?;
            let file = fs_user_get_node(task, socket_fd).ok_or(ENOSYS)?;
            file.handlers = &SOCKET_V6_HANDLERS;
            Ok(socket_fd)
        }

        AF_INET => {
            let cloexec = (ty & SOCK_CLOEXEC) != 0;
            let nonblock = (ty & SOCK_NONBLOCK) != 0;
            ty &= !(SOCK_CLOEXEC | SOCK_NONBLOCK);

            let lwip_fd = lwip_socket(family, ty, protocol).map_err(|e| -e)?;
            assert!(lwip_fcntl(lwip_fd, F_SETFL, O_NONBLOCK) == 0);

            let socket_fd = fs_user_open(task, "/dev/stdout", O_RDWR, 0)?;
            let socket_node = fs_user_get_node(task, socket_fd).ok_or(-1)?;

            if cloexec { socket_node.close_on_exec = true; }
            if nonblock { socket_node.flags |= O_NONBLOCK; }

            socket_node.handlers = &SOCKET_HANDLERS;

            let user_socket = Box::into_raw(Box::new(UserSocket {
                lwip_fd,
                socket_instances: 1,
            }));
            socket_node.dir = user_socket as *mut _;

            Ok(socket_fd)
        }

        _ => Err(ENOSYS),
    }
}

// ==========================
// Generic syscall helper
// ==========================
macro_rules! dispatch_file_handler {
    ($task:ident, $fd:expr, $handler:ident, $( $arg:expr ),* ) => {{
        let file = fs_user_get_node($task, $fd).ok_or(EBADF)?;
        if file.handlers.$handler.is_none() {
            return Err(ENOTSOCK);
        }
        file.handlers.$handler.unwrap()(file, $($arg),*)
    }};
}

// ==========================
// Syscalls
// ==========================
pub fn syscall_connect(task: &mut Task, fd: i32, addr: &sockaddr_linux, len: usize) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, connect, addr, len)
}

pub fn syscall_accept(task: &mut Task, fd: i32, addr: &mut sockaddr_linux, len: &mut u32) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, accept, addr, len)
}

pub fn syscall_bind(task: &mut Task, fd: i32, addr: &sockaddr_linux, len: usize) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, bind, addr, len)
}

pub fn syscall_listen(task: &mut Task, fd: i32, backlog: i32) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, listen, backlog)
}

pub fn syscall_getsockname(task: &mut Task, fd: i32, addr: &mut sockaddr_linux, len: &mut socklen_t) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, getsockname, addr, len)
}

pub fn syscall_getpeername(task: &mut Task, fd: i32, addr: &mut sockaddr_linux, len: &mut socklen_t) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, getpeername, addr, len)
}

pub fn syscall_socketpair(task: &mut Task, family: i32, ty: i32, protocol: i32, sv: &mut [i32;2]) -> Result<usize, i32> {
    match family {
        AF_UNIX => unix_socket_pair(ty, protocol, sv),
        _ => Err(ENOSYS),
    }
}

pub fn syscall_getsockopt(task: &mut Task, fd: i32, level: i32, optname: i32, optval: *mut u8, socklen: &mut u32) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, getsockopts, level, optname, optval, socklen)
}

pub fn syscall_sendto(task: &mut Task, fd: i32, buff: *const u8, len: usize, flags: i32, addr: &sockaddr_linux, addrlen: socklen_t) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, sendto, buff, len, flags, addr, addrlen)
}

pub fn syscall_recvfrom(task: &mut Task, fd: i32, buff: *mut u8, len: usize, flags: i32, addr: &mut sockaddr_linux, addrlen: &mut socklen_t) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, recvfrom, buff, len, flags, addr, addrlen)
}

pub fn syscall_sendmsg(task: &mut Task, fd: i32, msg: &msghdr_linux, flags: i32) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, sendmsg, msg, flags)
}

pub fn syscall_recvmsg(task: &mut Task, fd: i32, msg: &mut msghdr_linux, flags: i32) -> Result<usize, i32> {
    dispatch_file_handler!(task, fd, recvmsg, msg, flags)
}

// ==========================
// Register all network syscalls
// ==========================
pub fn syscalls_reg_net() {
    register_syscall(SYSCALL_SOCKET, syscall_socket as usize);
    register_syscall(SYSCALL_SOCKETPAIR, syscall_socketpair as usize);
    register_syscall(SYSCALL_CONNECT, syscall_connect as usize);
    register_syscall(SYSCALL_ACCEPT, syscall_accept as usize);
    register_syscall(SYSCALL_BIND, syscall_bind as usize);
    register_syscall(SYSCALL_SENDTO, syscall_sendto as usize);
    register_syscall(SYSCALL_RECVFROM, syscall_recvfrom as usize);
    register_syscall(SYSCALL_RECVMSG, syscall_recvmsg as usize);
    register_syscall(SYSCALL_SENDMSG, syscall_sendmsg as usize);
    register_syscall(SYSCALL_LISTEN, syscall_listen as usize);
    register_syscall(SYSCALL_GETSOCKOPT, syscall_getsockopt as usize);
    register_syscall(SYSCALL_GETPEERNAME, syscall_getpeername as usize);
    register_syscall(SYSCALL_GETSOCKNAME, syscall_getsockname as usize);
}
