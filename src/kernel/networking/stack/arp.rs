use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const IPV4_BYTE_SIZE: usize = 4;
const MAC_BYTE_SIZE: usize = 6;
const ARP_HARDWARE_TYPE: u16 = 1;
const ARP_PROTOCOL_TYPE: u16 = 0x0800;
const ARP_OP_REQUEST: u16 = 1;
const ARP_OP_REPLY: u16 = 2;

// Broadcast MAC address
const MAC_BROADCAST: [u8; MAC_BYTE_SIZE] = [0xff; MAC_BYTE_SIZE];
const ADDRESS_NULL: [u8; IPV4_BYTE_SIZE] = [0; IPV4_BYTE_SIZE];

#[repr(C)]
#[derive(Clone)]
struct ArpEntry {
    mac: [u8; MAC_BYTE_SIZE],
    ip: [u8; IPV4_BYTE_SIZE],
}

#[repr(C)]
struct ArpPacket {
    hardware_type: u16,
    protocol_type: u16,
    hardware_size: u8,
    protocol_size: u8,
    opcode: u16,
    sender_mac: [u8; MAC_BYTE_SIZE],
    sender_ip: [u8; IPV4_BYTE_SIZE],
    target_mac: [u8; MAC_BYTE_SIZE],
    target_ip: [u8; IPV4_BYTE_SIZE],
}

struct ArpTable {
    entries: Vec<ArpEntry>,
}

impl ArpTable {
    fn new() -> Self {
        Self { entries: Vec::new() }
    }

    fn lookup(&self, ip: &[u8; IPV4_BYTE_SIZE]) -> Option<[u8; MAC_BYTE_SIZE]> {
        self.entries.iter()
            .find(|entry| &entry.ip == ip)
            .map(|entry| entry.mac)
    }

    fn add(&mut self, mac: [u8; MAC_BYTE_SIZE], ip: [u8; IPV4_BYTE_SIZE]) {
        // Remove any conflicting entry
        if let Some(pos) = self.entries.iter().position(|e| &e.ip == &ip || &e.mac == &mac) {
            self.entries.remove(pos);
        }
        self.entries.push(ArpEntry { mac, ip });
    }
}

struct ArpRequest {
    ip: [u8; IPV4_BYTE_SIZE],
    timestamp: Instant,
}

struct NetQueueItem {
    component: u32,
    callback: Option<fn()>,
    dir: Box<dyn std::any::Any + Send>, // stores ARP request
    unix_time: Instant,
}

struct NetQueue {
    queue: VecDeque<NetQueueItem>,
}

impl NetQueue {
    fn new() -> Self {
        Self { queue: VecDeque::new() }
    }

    fn search<F>(&self, cmp: F) -> Option<&NetQueueItem>
    where F: Fn(&NetQueueItem) -> bool {
        self.queue.iter().find(|item| cmp(item))
    }

    fn remove<F>(&mut self, cmp: F) -> bool
    where F: Fn(&NetQueueItem) -> bool {
        if let Some(pos) = self.queue.iter().position(|item| cmp(item)) {
            self.queue.remove(pos);
            true
        } else { false }
    }
}

struct NIC {
    mac: [u8; MAC_BYTE_SIZE],
    ip: [u8; IPV4_BYTE_SIZE],
    arp_table: Mutex<ArpTable>,
}

impl NIC {
    fn arp_table_lookup(&self, ip: &[u8; IPV4_BYTE_SIZE]) -> Option<[u8; MAC_BYTE_SIZE]> {
        self.arp_table.lock().unwrap().lookup(ip)
    }

    fn arp_table_add(&self, mac: [u8; MAC_BYTE_SIZE], ip: [u8; IPV4_BYTE_SIZE]) {
        self.arp_table.lock().unwrap().add(mac, ip);
    }

    fn arp_reply(&self, req_mac: [u8; MAC_BYTE_SIZE], req_ip: [u8; IPV4_BYTE_SIZE]) {
        let arp = ArpPacket {
            hardware_type: ARP_HARDWARE_TYPE.to_be(),
            protocol_type: ARP_PROTOCOL_TYPE.to_be(),
            hardware_size: MAC_BYTE_SIZE as u8,
            protocol_size: IPV4_BYTE_SIZE as u8,
            opcode: ARP_OP_REPLY.to_be(),
            target_mac: req_mac,
            target_ip: req_ip,
            sender_mac: self.mac,
            sender_ip: self.ip,
        };

        // send the packet (placeholder)
        net_eth_send(&arp, &req_mac);
    }

    fn arp_translate(&self, ip: [u8; IPV4_BYTE_SIZE], queue: &Mutex<NetQueue>) -> Option<[u8; MAC_BYTE_SIZE]> {
        // Lookup in ARP table
        if let Some(mac) = self.arp_table_lookup(&ip) {
            return Some(mac);
        }

        // Check queue
        let mut net_queue = queue.lock().unwrap();
        let existing = net_queue.search(|item| {
            item.component == 1 && item.dir.downcast_ref::<ArpRequest>().map_or(false, |r| r.ip == ip)
        });

        let send_request = if let Some(item) = existing {
            item.unix_time.elapsed() > Duration::from_millis(500)
        } else {
            true
        };

        if send_request {
            let request = ArpRequest {
                ip,
                timestamp: Instant::now(),
            };
            net_queue.queue.push_back(NetQueueItem {
                component: 1,
                callback: None,
                dir: Box::new(request),
                unix_time: Instant::now(),
            });

            // Build and send ARP request
            let arp = ArpPacket {
                hardware_type: ARP_HARDWARE_TYPE.to_be(),
                protocol_type: ARP_PROTOCOL_TYPE.to_be(),
                hardware_size: MAC_BYTE_SIZE as u8,
                protocol_size: IPV4_BYTE_SIZE as u8,
                opcode: ARP_OP_REQUEST.to_be(),
                target_mac: [0; MAC_BYTE_SIZE],
                target_ip: ip,
                sender_mac: self.mac,
                sender_ip: self.ip,
            };
            net_eth_send(&arp, &MAC_BROADCAST);
        }

        None
    }
}

// placeholder for sending Ethernet frame
fn net_eth_send(packet: &ArpPacket, dest_mac: &[u8; MAC_BYTE_SIZE]) {
    println!("Sending ARP packet to {:02x?}", dest_mac);
}
