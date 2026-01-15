//! Network Interface Sequential API module
//!
//! Thread-safe functions to be called from non-TCPIP threads.

use core::ptr;
use std::sync::{Arc, Mutex};

#[cfg(feature = "ipv4")]
use crate::ip4::{Ip4Addr, IP4_ADDR_ANY};
use crate::netif::{NetIf, NetIfInitFn, NetIfInputFn};
use crate::tcpip::tcpip_api_call;
use crate::err::{ErrT, ERR_OK, ERR_IF};

/// Function pointers used for common netif operations
pub type NetIfVoidFn = fn(&mut NetIf);
pub type NetIfErrtFn = fn(&mut NetIf) -> ErrT;

/// Internal message for API calls
struct NetIfApiMsg<'a> {
    netif: &'a mut NetIf,
    msg: NetIfApiMsgUnion<'a>,
}

enum NetIfApiMsgUnion<'a> {
    Add {
        #[cfg(feature = "ipv4")]
        ipaddr: &'a Ip4Addr,
        #[cfg(feature = "ipv4")]
        netmask: &'a Ip4Addr,
        #[cfg(feature = "ipv4")]
        gw: &'a Ip4Addr,
        state: Option<*mut core::ffi::c_void>,
        init: NetIfInitFn,
        input: NetIfInputFn,
    },
    Common {
        voidfunc: NetIfVoidFn,
        errtfunc: Option<NetIfErrtFn>,
    },
    Ifs {
        name: &'a str,
        index: u8,
    },
}

/// Calls netif_add inside the tcpip_thread context
fn netifapi_do_netif_add(msg: &mut NetIfApiMsg) -> ErrT {
    match &msg.msg {
        NetIfApiMsgUnion::Add { #[cfg(feature = "ipv4")] ipaddr,
                                #[cfg(feature = "ipv4")] netmask,
                                #[cfg(feature = "ipv4")] gw,
                                state,
                                init,
                                input } => {
            if !msg.netif.add(
                #[cfg(feature = "ipv4")]
                ipaddr,
                #[cfg(feature = "ipv4")]
                netmask,
                #[cfg(feature = "ipv4")]
                gw,
                *state,
                *init,
                *input,
            ) {
                ERR_IF
            } else {
                ERR_OK
            }
        }
        _ => ERR_IF,
    }
}

/// Calls netif_set_addr inside tcpip_thread context
#[cfg(feature = "ipv4")]
fn netifapi_do_netif_set_addr(msg: &mut NetIfApiMsg) -> ErrT {
    if let NetIfApiMsgUnion::Add { ipaddr, netmask, gw, .. } = &msg.msg {
        msg.netif.set_addr(ipaddr, netmask, gw);
        ERR_OK
    } else {
        ERR_IF
    }
}

/// Calls "errtfunc" or "voidfunc" inside tcpip_thread context
fn netifapi_do_netif_common(msg: &mut NetIfApiMsg) -> ErrT {
    if let NetIfApiMsgUnion::Common { voidfunc, errtfunc } = &msg.msg {
        if let Some(f) = errtfunc {
            f(msg.netif)
        } else {
            voidfunc(msg.netif);
            ERR_OK
        }
    } else {
        ERR_IF
    }
}

/// Thread-safe wrapper for netif_add
pub fn netifapi_netif_add(
    netif: &mut NetIf,
    #[cfg(feature = "ipv4")] ipaddr: Option<&Ip4Addr>,
    #[cfg(feature = "ipv4")] netmask: Option<&Ip4Addr>,
    #[cfg(feature = "ipv4")] gw: Option<&Ip4Addr>,
    state: Option<*mut core::ffi::c_void>,
    init: NetIfInitFn,
    input: NetIfInputFn,
) -> ErrT {
    let mut msg = NetIfApiMsg {
        netif,
        msg: NetIfApiMsgUnion::Add {
            #[cfg(feature = "ipv4")]
            ipaddr: ipaddr.unwrap_or(&IP4_ADDR_ANY),
            #[cfg(feature = "ipv4")]
            netmask: netmask.unwrap_or(&IP4_ADDR_ANY),
            #[cfg(feature = "ipv4")]
            gw: gw.unwrap_or(&IP4_ADDR_ANY),
            state,
            init,
            input,
        },
    };
    tcpip_api_call(|| netifapi_do_netif_add(&mut msg))
}

/// Thread-safe wrapper for netif_set_addr
#[cfg(feature = "ipv4")]
pub fn netifapi_netif_set_addr(
    netif: &mut NetIf,
    ipaddr: Option<&Ip4Addr>,
    netmask: Option<&Ip4Addr>,
    gw: Option<&Ip4Addr>,
) -> ErrT {
    let mut msg = NetIfApiMsg {
        netif,
        msg: NetIfApiMsgUnion::Add {
            ipaddr: ipaddr.unwrap_or(&IP4_ADDR_ANY),
            netmask: netmask.unwrap_or(&IP4_ADDR_ANY),
            gw: gw.unwrap_or(&IP4_ADDR_ANY),
            state: None,
            init: NetIf::dummy_init,
            input: NetIf::dummy_input,
        },
    };
    tcpip_api_call(|| netifapi_do_netif_set_addr(&mut msg))
}

/// Thread-safe wrapper for calling common netif functions
pub fn netifapi_netif_common(
    netif: &mut NetIf,
    voidfunc: NetIfVoidFn,
    errtfunc: Option<NetIfErrtFn>,
) -> ErrT {
    let mut msg = NetIfApiMsg {
        netif,
        msg: NetIfApiMsgUnion::Common { voidfunc, errtfunc },
    };
    tcpip_api_call(|| netifapi_do_netif_common(&mut msg))
}
