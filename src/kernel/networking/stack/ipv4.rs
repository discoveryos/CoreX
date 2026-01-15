use std::convert::TryInto;
use std::mem;
use std::ptr;

// Constants
const IPV4_BYTE_SIZE: usize = 4;
const MAC_BYTE_SIZE: usize = 6;
const NET_IPv4_CARRY: usize = 14; // Ethernet header size
const IPV4_FLAGS_MORE_FRAGMENTS: u16 = 0x2000;

const IPV4_PROTOCOL_TCP: u8 = 6;
const IPV4_PROTOCOL_UDP: u8 = 17;
const IPV4_PROTOCOL_ICMP: u8 = 1;

// Placeholder external task
static mut NET_HELPER_TASK: *const Task = ptr::null();

// IPv4 header structure
#[repr(C)]
#[derive(Clone, Copy)]
struct IPv4Header {
    version_ihl: u8,     // version (4 bits) + IHL (4 bits)
    tos: u8,
    length: u16,
    id: u16,
    flags_fragment: u16, // flags + fragment offset
    ttl: u8,
    protocol: u8,
    checksum: u16,
    src_address: [u8; IPV4_BYTE_SIZE],
    dest_address: [u8; IPV4_BYTE_SIZE],
}

impl IPv4Header {
    fn version(&self) -> u8 {
        self.version_ihl >> 4
    }
    fn ihl(&self) -> u8 {
        self.version_ihl & 0x0F
    }
    fn set_ihl(&mut self, ihl: u8) {
        self.version_ihl = (self.version() << 4) | (ihl & 0x0F);
    }
}

// NIC struct
struct NIC {
    ip: [u8; IPV4_BYTE_SIZE],
    subnet_mask: [u8; IPV4_BYTE_SIZE],
    server_ip: [u8; IPV4_BYTE_SIZE],
    mac: [u8; MAC_BYTE_SIZE],
    mtu: usize,
}

// Placeholder current task
static mut CURRENT_TASK: *const Task = ptr::null();

// Placeholder Task struct
struct Task;

// Utility: checksum calculation
fn ipv4_checksum(buf: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;

    while i + 1 < buf.len() {
        let word = u16::from_be_bytes(buf[i..i + 2].try_into().unwrap()) as u32;
        sum = sum.wrapping_add(word);
        i += 2;
    }

    if i < buf.len() {
        sum = sum.wrapping_add((buf[i] as u32) << 8);
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

// Determine if routing is needed
fn ipv4_needs_routing(local: &[u8; 4], dest: &[u8; 4], mask: &[u8; 4]) -> bool {
    let local_ip = u32::from_be_bytes(*local);
    let dest_ip = u32::from_be_bytes(*dest);
    let mask_ip = u32::from_be_bytes(*mask);
    (dest_ip & mask_ip) != (local_ip & mask_ip)
}

// Handle incoming IPv4 packets
fn net_ipv4_handle(nic: &NIC, packet: &[u8]) {
    if packet.len() < NET_IPv4_CARRY + mem::size_of::<IPv4Header>() {
        println!("[net::ipv4] Drop: Too small");
        return;
    }

    let ipv4 = unsafe { &*(packet[NET_IPv4_CARRY..].as_ptr() as *const IPv4Header) };

    if ipv4.version() != 4 {
        println!("[net::ipv4] Drop: IP version != 4");
        return;
    }

    let flags_fragment = u16::from_be(ipv4.flags_fragment);
    if (flags_fragment & IPV4_FLAGS_MORE_FRAGMENTS) != 0
        || (flags_fragment & 0x1FFF) != 0
    {
        println!("[net::ipv4] Drop: Fragmentation not supported");
        return;
    }

    let length = u16::from_be(ipv4.length) as usize;
    if length > packet.len() - NET_IPv4_CARRY {
        println!("[net::ipv4] Drop: Invalid IPv4 length");
        return;
    }

    let header_len = ipv4.ihl() as usize * 4;
    if header_len < mem::size_of::<IPv4Header>() {
        println!("[net::ipv4] Drop: IHL smaller than header");
        return;
    }

    if ipv4.dest_address != nic.ip
        && ipv4.dest_address != [0xff; 4]
        && ipv4.dest_address != [0x00; 4]
    {
        println!("[net::ipv4] Drop: Not for us or broadcast");
        return;
    }

    if ipv4.ttl == 1 {
        println!("[net::ipv4] Drop: TTL would reach 0");
        return;
    }

    let header_bytes = &packet[NET_IPv4_CARRY..NET_IPv4_CARRY + header_len];
    if ipv4_checksum(header_bytes) != 0 {
        println!("[net::ipv4] Drop: Checksum failed");
        return;
    }

    match ipv4.protocol {
        IPV4_PROTOCOL_UDP => net_udp_handle(nic, packet),
        IPV4_PROTOCOL_TCP => (),  // TCP handling not implemented yet
        IPV4_PROTOCOL_ICMP => (), // ICMP handling not implemented yet
        _ => println!("[net::ipv4] Drop: Unhandled protocol"),
    }
}

// Initialize IPv4 buffer
fn ipv4_init_buffer(buf: &mut [u8]) {
    assert!(buf.len() >= NET_IPv4_CARRY + mem::size_of::<IPv4Header>());
    let ipv4 = unsafe { &mut *(buf[NET_IPv4_CARRY..].as_mut_ptr() as *mut IPv4Header) };
    ipv4.set_ihl((mem::size_of::<IPv4Header>() / 4) as u8);
}

// Send IPv4 packet
fn net_ipv4_send(nic: &NIC, packet: &mut [u8], protocol: u8, dest_ip: &[u8; 4]) {
    assert!(packet.len() >= NET_IPv4_CARRY + mem::size_of::<IPv4Header>());
    let ipv4 = unsafe { &mut *(packet[NET_IPv4_CARRY..].as_mut_ptr() as *mut IPv4Header) };
    assert!(ipv4.ihl() as usize == mem::size_of::<IPv4Header>() / 4);

    // Clear header
    *ipv4 = IPv4Header {
        version_ihl: 0,
        tos: 0,
        length: 0,
        id: 0,
        flags_fragment: 0,
        ttl: 255,
        protocol,
        checksum: 0,
        src_address: nic.ip,
        dest_address: *dest_ip,
    };
    ipv4.set_ihl((mem::size_of::<IPv4Header>() / 4) as u8);

    ipv4.length = ((packet.len() - NET_IPv4_CARRY) as u16).to_be();
    ipv4.id = rand::random::<u16>().to_be();

    ipv4.checksum = 0;
    let header_bytes =
        &mut packet[NET_IPv4_CARRY..NET_IPv4_CARRY + (ipv4.ihl() as usize * 4)];
    ipv4.checksum = ipv4_checksum(header_bytes);

    // Determine routing
    let route_ip = if ipv4_needs_routing(&nic.ip, dest_ip, &nic.subnet_mask) {
        nic.server_ip
    } else {
        *dest_ip
    };

    let mut dest_mac = [0u8; MAC_BYTE_SIZE];
    unsafe {
        if CURRENT_TASK != NET_HELPER_TASK {
            while !net_arp_translate(nic, &route_ip, &mut dest_mac) {
                hand_control();
            }
        } else {
            panic!("NET_HELPER_TASK path not implemented");
        }
    }

    net_eth_send(nic, packet, &dest_mac, 0x0800); // 0x0800 = IPv4
}

// Placeholder UDP handler
fn net_udp_handle(_nic: &NIC, _packet: &[u8]) {
    println!("[net::udp] Packet handled");
}

// Placeholder ARP functions
fn net_arp_translate(_nic: &NIC, _ip: &[u8; 4], _mac: &mut [u8; 6]) -> bool {
    true
}

// Placeholder functions
fn hand_control() {}
fn net_eth_send(_nic: &NIC, _packet: &mut [u8], _dest_mac: &[u8; 6], _ethertype: u16) {}
