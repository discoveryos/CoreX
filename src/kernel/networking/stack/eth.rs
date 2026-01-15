use std::sync::Arc;

// Constants
const MAC_BYTE_SIZE: usize = 6;
const IPV4_BYTE_SIZE: usize = 4;
const NET_ETHERTYPE_ARP: u16 = 0x0806;
const NET_ETHERTYPE_IPV4: u16 = 0x0800;
const NET_ETHERTYPE_IPV6: u16 = 0x86DD;

// Broadcast / zero addresses
const MAC_BROADCAST: [u8; MAC_BYTE_SIZE] = [0xff; MAC_BYTE_SIZE];
const MAC_ZERO: [u8; MAC_BYTE_SIZE] = [0x00; MAC_BYTE_SIZE];
const ADDR_NULL: [u8; IPV4_BYTE_SIZE] = [0x00; IPV4_BYTE_SIZE];
const ADDR_BROADCAST: [u8; IPV4_BYTE_SIZE] = [0xff; IPV4_BYTE_SIZE];

// Ethernet header
#[repr(C)]
#[derive(Clone, Copy)]
struct EthHeader {
    dest: [u8; MAC_BYTE_SIZE],
    src: [u8; MAC_BYTE_SIZE],
    ethertype: u16,
}

// Debug helpers
fn dbg_mac(mac: &[u8; MAC_BYTE_SIZE]) {
    println!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
}

fn dbg_ip(ip: &[u8; IPV4_BYTE_SIZE]) {
    println!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
}

// NIC struct
struct NIC {
    mac: [u8; MAC_BYTE_SIZE],
    mtu: usize,
    // ARP, IPv4, etc.
}

impl NIC {
    fn net_eth_handle(&self, packet: &[u8]) {
        if packet.len() < std::mem::size_of::<EthHeader>() {
            println!("[net::eth] Drop: Too small");
            return;
        }

        let eth = unsafe { &*(packet.as_ptr() as *const EthHeader) };

        if eth.dest != self.mac && eth.dest != MAC_BROADCAST && eth.dest != MAC_ZERO {
            println!("[net::eth] Drop: Neither ours nor broadcast");
            return;
        }

        match u16::from_be(eth.ethertype) {
            NET_ETHERTYPE_ARP => {
                net_arp_handle(self, packet);
            }
            NET_ETHERTYPE_IPV4 => {
                net_ipv4_handle(self, packet);
            }
            NET_ETHERTYPE_IPV6 => {
                // ignored
            }
            _ => {
                println!("[net::eth] Drop: Unhandled ethertype");
            }
        }
    }

    fn net_eth_send(&self, packet: &mut [u8], target_mac: &[u8; MAC_BYTE_SIZE], ethertype: u16) {
        assert!(packet.len() >= std::mem::size_of::<EthHeader>());
        assert!(packet.len() <= self.mtu);

        let eth = unsafe { &mut *(packet.as_mut_ptr() as *mut EthHeader) };
        eth.src.copy_from_slice(&self.mac);
        eth.dest.copy_from_slice(target_mac);
        eth.ethertype = ethertype.to_be();

        send_packet_raw(self, packet);
    }
}

// Placeholder functions for ARP and IPv4 handlers
fn net_arp_handle(_nic: &NIC, _packet: &[u8]) {
    println!("[net::eth] ARP packet handled");
}

fn net_ipv4_handle(_nic: &NIC, _packet: &[u8]) {
    println!("[net::eth] IPv4 packet handled");
}

// Placeholder raw send function
fn send_packet_raw(_nic: &NIC, _packet: &[u8]) {
    println!("[net::eth] Raw packet sent");
}
