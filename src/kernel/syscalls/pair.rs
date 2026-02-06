use crate::*;
use super::types::*;

pub unsafe fn unix_socket_allocate_pair() -> *mut UnixSocketPair {
    let pair = calloc(core::mem::size_of::<UnixSocketPair>(), 1)
        as *mut UnixSocketPair;

    (*pair).client_buff_size = UNIX_SOCK_BUFF_DEFAULT;
    (*pair).server_buff_size = UNIX_SOCK_BUFF_DEFAULT;

    CircularAllocate(&mut (*pair).client_buff, (*pair).client_buff_size);
    CircularAllocate(&mut (*pair).server_buff, (*pair).server_buff_size);

    pair
}

pub unsafe fn unix_socket_free_pair(pair: *mut UnixSocketPair) {
    assert!((*pair).client_fds == 0 && (*pair).server_fds == 0);

    CircularFree(&mut (*pair).client_buff);
    CircularFree(&mut (*pair).server_buff);
    free((*pair).filename as _);
    free(pair as _);
}

/* accept() fd creation */

pub unsafe fn unix_socket_accept_create(pair: *mut UnixSocketPair) -> *mut OpenFile {
    let fd = fsUserOpen(currentTask, b"/dev/null\0".as_ptr(), O_RDWR, 0);
    assert!(!RET_IS_ERR(fd));

    let node = fsUserGetNode(currentTask, fd);
    (*node).dir = pair as _;
    (*node).handlers = &unixAcceptHandlers;
    node
}
