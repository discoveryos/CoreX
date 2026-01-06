#![no_std]

use core::ptr::{null_mut};
use core::sync::atomic::{AtomicI32, Ordering};

//
// ================= Externs =================
//

extern "C" {
    fn debugf(fmt: *const u8, ...) -> i32;
    fn panic() -> !;

    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
    fn memcpy(dst: *mut u8, src: *const u8, len: usize);
    fn memset(dst: *mut u8, val: i32, len: usize);

    // PCI / NIC
    fn initiateNe2000(dev: *mut PCIdevice) -> bool;
    fn initiateRTL8139(dev: *mut PCIdevice) -> bool;
    fn initiateRTL8169(dev: *mut PCIdevice) -> bool;
    fn initiateE1000(dev: *mut PCIdevice) -> bool;

    fn sendNe2000(nic: *mut NIC, data: *const u8, size: u32);
    fn sendRTL8139(nic: *mut NIC, data: *const u8, size: u32);
    fn sendRTL8169(nic: *mut NIC, data: *const u8, size: u32);
    fn sendE1000(nic: *mut NIC, data: *const u8, size: u32);

    // lwIP
    fn tcpip_init(init: extern "C" fn(*mut core::ffi::c_void), arg: *mut core::ffi::c_void);
    fn tcpip_input(p: *mut pbuf, netif: *mut netif) -> err_t;
    fn netif_add(
        netif: *mut netif,
        ip: *const ip4_addr,
        mask: *const ip4_addr,
        gw: *const ip4_addr,
        state: *mut core::ffi::c_void,
        init: extern "C" fn(*mut netif) -> err_t,
        input: extern "C" fn(*mut pbuf, *mut netif) -> err_t,
    );
    fn netif_set_default(netif: *mut netif);
    fn netif_set_up(netif: *mut netif);
    fn dhcp_start(netif: *mut netif) -> err_t;
    fn dhcp_supplied_address(netif: *mut netif) -> bool;
    fn sys_check_timeouts();
    fn etharp_output(netif: *mut netif, p: *mut pbuf, ipaddr: *const ip4_addr) -> err_t;

    fn pbuf_alloc(layer: u8, len: u16, r#type: u8) -> *mut pbuf;

    static mut selectedNIC: *mut NIC;
    static mut dsPCI: LinkedList;
}

//
// ================= lwIP types =================
//

pub type err_t = i32;

pub const ERR_OK: err_t = 0;

pub const PBUF_RAW: u8 = 0;
pub const PBUF_RAM: u8 = 0;

pub const NETIF_FLAG_BROADCAST: u8 = 1 << 0;
pub const NETIF_FLAG_ETHARP: u8 = 1 << 1;
pub const NETIF_FLAG_ETHERNET: u8 = 1 << 2;
pub const NETIF_FLAG_LINK_UP: u8 = 1 << 3;

pub const ETHARP_HWADDR_LEN: u8 = 6;

#[repr(C)]
pub struct ip4_addr {
    pub addr: u32,
}

#[repr(C)]
pub struct netif {
    pub state: *mut core::ffi::c_void,
    pub name: [u8; 2],
    pub next: *mut netif,
    pub output: Option<extern "C" fn(*mut netif, *mut pbuf, *const ip4_addr) -> err_t>,
    pub linkoutput: Option<extern "C" fn(*mut netif, *mut pbuf) -> err_t>,
    pub input: extern "C" fn(*mut pbuf, *mut netif) -> err_t,
    pub hwaddr_len: u8,
    pub hwaddr: [u8; 6],
    pub mtu: u16,
    pub flags: u8,
}

#[repr(C)]
pub struct pbuf {
    pub next: *mut pbuf,
    pub payload: *mut u8,
    pub len: u16,
    pub tot_len: u16,
}

//
// ================= Kernel structs =================
//

#[repr(C)]
pub struct PCIdevice {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
}

#[repr(C)]
pub struct PCI {
    pub category: u32,
    pub extra: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct LinkedList {
    _unused: u8,
}

#[repr(C)]
pub struct NIC {
    pub r#type: u32,
    pub mtu: u16,
    pub MAC: [u8; 6],
    pub infoLocation: *mut core::ffi::c_void,
    pub lwip: netif,
}

//
// ================= Constants =================
//

pub const PCI_DRIVER_CATEGORY_NIC: u32 = 2;

pub const NE2000: u32 = 1;
pub const RTL8139: u32 = 2;
pub const RTL8169: u32 = 3;
pub const E1000: u32 = 4;

//
// ================= lwIP glue =================
//

#[no_mangle]
pub extern "C" fn lwipDummyInit(_netif: *mut netif) -> err_t {
    ERR_OK
}

//
// ================= Networking init =================
//

#[no_mangle]
pub extern "C" fn initiateNetworking() {
    unsafe {
        selectedNIC = null_mut();
        debugf(b"[networking] Ready to scan for NICs..\n\0".as_ptr());
    }
}

//
// ================= lwIP output =================
//

#[no_mangle]
pub extern "C" fn lwipOutput(netif: *mut netif, p: *mut pbuf) -> err_t {
    unsafe {
        let total = (*p).tot_len as usize;
        let buf = malloc(total) as *mut u8;

        let mut cur = p;
        let mut off = 0usize;
        while !cur.is_null() {
            memcpy(
                buf.add(off),
                (*cur).payload,
                (*cur).len as usize,
            );
            off += (*cur).len as usize;
            cur = (*cur).next;
        }

        if off != total {
            debugf(b"[networking::lwipOut] Corrupted pbuf!\n\0".as_ptr());
            panic();
        }

        let pci = LinkedListSearch(&mut dsPCI, lwipOutputCb, netif);
        if pci.is_null() {
            debugf(b"[nics] Couldn't find netif!\n\0".as_ptr());
            panic();
        }

        let nic = (*(pci as *mut PCI)).extra as *mut NIC;
        sendPacketRaw(nic, buf, total as u32);
        free(buf);

        ERR_OK
    }
}

extern "C" fn lwipOutputCb(data: *mut core::ffi::c_void,
                           ctx: *mut core::ffi::c_void) -> bool {
    unsafe {
        let pci = data as *mut PCI;
        let nic = (*pci).extra as *mut NIC;
        (*pci).category == PCI_DRIVER_CATEGORY_NIC && &(*nic).lwip as *const _ == ctx as *const _
    }
}

//
// ================= lwIP thread =================
//

#[no_mangle]
pub extern "C" fn lwipInitInThread(arg: *mut core::ffi::c_void) {
    unsafe {
        let nic = arg as *mut NIC;
        let netif = &mut (*nic).lwip;

        netif.state = null_mut();
        netif.name = [b'A', b'B'];
        netif.next = null_mut();

        let mask = ip4_addr { addr: 0xFFFFFF00 };

        netif_add(
            netif,
            null_mut(),
            &mask,
            null_mut(),
            null_mut(),
            lwipDummyInit,
            tcpip_input,
        );

        netif.output = Some(etharp_output);
        netif.linkoutput = Some(lwipOutput);
        netif_set_default(netif);

        netif.hwaddr_len = ETHARP_HWADDR_LEN;
        netif.hwaddr = (*nic).MAC;
        netif.mtu = (*nic).mtu;
        netif.flags = NETIF_FLAG_BROADCAST
            | NETIF_FLAG_ETHARP
            | NETIF_FLAG_ETHERNET
            | NETIF_FLAG_LINK_UP;

        netif_set_up(netif);

        if dhcp_start(netif) != ERR_OK {
            debugf(b"[nic::lwip] DHCP failed!\n\0".as_ptr());
            panic();
        }

        sys_check_timeouts();
        dhcp_supplied_address(netif);
    }
}

//
// ================= NIC detection =================
//

#[no_mangle]
pub extern "C" fn initiateNIC(device: *mut PCIdevice) {
    unsafe {
        if initiateNe2000(device)
            || initiateRTL8139(device)
            || initiateRTL8169(device)
            || initiateE1000(device)
        {
            tcpip_init(lwipInitInThread, selectedNIC as *mut _);
        }
    }
}

//
// ================= NIC creation =================
//

#[no_mangle]
pub extern "C" fn createNewNIC(pci: *mut PCI) -> *mut NIC {
    unsafe {
        let nic = malloc(core::mem::size_of::<NIC>()) as *mut NIC;
        memset(nic as *mut u8, 0, core::mem::size_of::<NIC>());
        (*nic).mtu = 1500;
        (*pci).extra = nic as *mut _;
        selectedNIC = nic;
        nic
    }
}

//
// ================= Packet send =================
//

#[repr(C)]
struct netPacketHeader {
    destination_mac: [u8; 6],
    source_mac: [u8; 6],
    ethertype: u16,
}

#[no_mangle]
pub extern "C" fn sendPacketRaw(nic: *mut NIC, data: *const u8, size: u32) {
    unsafe {
        match (*nic).r#type {
            NE2000 => sendNe2000(nic, data, size),
            RTL8139 => sendRTL8139(nic, data, size),
            RTL8169 => sendRTL8169(nic, data, size),
            E1000 => sendE1000(nic, data, size),
            _ => {}
        }
    }
}

//
// ================= RX queue =================
//

pub const QUEUE_MAX: usize = 64;

#[repr(C)]
pub struct QueuePacket {
    pub nic: *mut NIC,
    pub packetLength: u16,
    pub buff: [u8; 2048],
}

static mut netQueue: [QueuePacket; QUEUE_MAX] = unsafe {
    core::mem::MaybeUninit::zeroed().assume_init()
};

static netQueueRead: AtomicI32 = AtomicI32::new(0);
static netQueueWrite: AtomicI32 = AtomicI32::new(0);

#[no_mangle]
pub extern "C" fn netQueueAdd(nic: *mut NIC, packet: *const u8, len: u16) {
    unsafe {
        let write = netQueueWrite.load(Ordering::Relaxed) as usize;
        let read = netQueueRead.load(Ordering::Relaxed) as usize;

        if (write + 1) % QUEUE_MAX == read {
            debugf(b"[netqueue] Packet dropped!\n\0".as_ptr());
            return;
        }

        let slot = &mut netQueue[write];
        slot.nic = nic;
        slot.packetLength = len;
        memcpy(slot.buff.as_mut_ptr(), packet, len as usize);

        netQueueWrite.store(((write + 1) % QUEUE_MAX) as i32, Ordering::Release);
    }
}
