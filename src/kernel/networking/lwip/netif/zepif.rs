//! Rust bindings for the lwIP ZEP (ZigBee Encapsulation Protocol) netif.
//!
//! This wraps `zepif.c` and allows sending/receiving 6LoWPAN over UDP.

use std::os::raw::{c_void, c_uchar, c_uint, c_ushort};
use std::ptr;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ZepHdr {
    pub prot_id: [c_uchar; 2],
    pub prot_version: c_uchar,
    pub zep_type: c_uchar,
    pub channel_id: c_uchar,
    pub device_id: c_ushort,
    pub crc_mode: c_uchar,
    pub unknown_1: c_uchar,
    pub timestamp: [c_uint; 2],
    pub seq_num: c_uint,
    pub unknown_2: [c_uchar; 10],
    pub len: c_uchar,
}

#[repr(C)]
#[derive(Debug)]
pub struct ZepifInit {
    pub zep_src_ip_addr: *mut c_void,
    pub zep_dst_ip_addr: *mut c_void,
    pub zep_src_udp_port: u16,
    pub zep_dst_udp_port: u16,
    pub zep_netif: *mut c_void,
    pub addr: [u8; 6],
}

#[repr(C)]
#[derive(Debug)]
pub struct Netif {
    pub state: *mut c_void,
    pub input: Option<extern "C" fn(*mut Netif, *mut c_void) -> i32>,
    pub hwaddr: [u8; 6],
    pub hwaddr_len: u8,
    pub linkoutput: Option<extern "C" fn(*mut Netif, *mut c_void) -> i32>,
}

extern "C" {
    /// Initialize a ZEPIF netif
    pub fn zepif_init(netif: *mut Netif) -> i32;
}

/// Safe wrapper for initializing a ZEPIF netif
pub fn init(netif: &mut Netif) -> Result<(), i32> {
    let ret = unsafe { zepif_init(netif as *mut Netif) };
    if ret == 0 {
        Ok(())
    } else {
        Err(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zepif_init() {
        let mut netif = Netif {
            state: ptr::null_mut(),
            input: None,
            hwaddr: [0; 6],
            hwaddr_len: 6,
            linkoutput: None,
        };

        // This will only work if lwIP is properly linked
        let result = init(&mut netif);
        println!("zepif_init result: {:?}", result);
    }
}
