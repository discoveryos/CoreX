use crate::*;
use super::socket::*;
use super::pair::*;

#[no_mangle]
pub static unixSocketHandlers: VfsHandlers = VfsHandlers {
    sendto: Some(unixSocketSendto),
    recvfrom: Some(unixSocketRecvfrom),
    bind: Some(unixSocketBind),
    listen: Some(unixSocketListen),
    accept: Some(unixSocketAccept),
    connect: Some(unixSocketConnect),
    getpeername: Some(unixSocketGetpeername),
    recvmsg: Some(unixSocketRecvmsg),
    sendmsg: Some(unixSocketSendmsg),
    duplicate: Some(unixSocketDuplicate),
    close: Some(unixSocketClose),
    reportKey: Some(unixSocketReportKey),
    internalPoll: Some(unixSocketInternalPoll),
};

#[no_mangle]
pub static unixAcceptHandlers: VfsHandlers = VfsHandlers {
    sendto: Some(unixSocketAcceptSendto),
    recvfrom: Some(unixSocketAcceptRecvfrom),
    recvmsg: Some(unixSocketAcceptRecvmsg),
    getpeername: Some(unixSocketAcceptGetpeername),
    duplicate: Some(unixSocketAcceptDuplicate),
    close: Some(unixSocketAcceptClose),
    reportKey: Some(unixSocketAcceptReportKey),
    internalPoll: Some(unixSocketAcceptInternalPoll),
};
