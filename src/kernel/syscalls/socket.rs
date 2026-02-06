use crate::*;
use super::types::*;
use super::pair::*;

pub unsafe fn unix_socket_open(task: *mut Task, ty: i32, proto: i32) -> usize {
    if (ty & 1) == 0 {
        return ERR(ENOSYS);
    }

    let fd = fsUserOpen(task, b"/dev/null\0".as_ptr(), O_RDWR, 0);
    assert!(!RET_IS_ERR(fd));

    let node = fsUserGetNode(task, fd);

    spinlockAcquire(&mut LOCK_LL_UNIX_SOCKET);
    let sock = LinkedListAllocate(&mut dsUnixSocket, core::mem::size_of::<UnixSocket>())
        as *mut UnixSocket;
    spinlockRelease(&mut LOCK_LL_UNIX_SOCKET);

    (*sock).times_opened = 1;
    (*node).dir = sock as _;
    (*node).handlers = &unixSocketHandlers;

    fd
}
