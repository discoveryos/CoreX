use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

// Constants
pub const IPv4_BYTE_SIZE: usize = 4;
pub const UDP_MAX_TOTAL_DATA: usize = 65535;

// Dummy structs for NIC, poll, etc.
pub struct NIC {
    pub udp: UdpStore,
    pub ip: [u8; IPv4_BYTE_SIZE],
}

pub struct UdpHeader {
    pub src_port: u16,
    pub dest_port: u16,
    pub length: u16,
}

pub struct IPv4Header {
    pub src_address: [u8; IPv4_BYTE_SIZE],
    pub dest_address: [u8; IPv4_BYTE_SIZE],
}

// UDP buffer
pub struct UdpBuffer {
    pub buff: Vec<u8>,
    pub len: usize,
    pub remote_port: u16,
    pub ip: [u8; IPv4_BYTE_SIZE],
}

// UDP connection
pub struct UdpConnection {
    pub local_port: u16,
    pub remote_port: u16,
    pub total_data: usize,
    pub ip: [u8; IPv4_BYTE_SIZE],
    pub ds_buffers: VecDeque<UdpBuffer>,
}

// UDP store
pub struct UdpStore {
    pub ds_udp_connections: Vec<Arc<Mutex<UdpConnection>>>,
}

impl UdpStore {
    pub fn new() -> Self {
        Self {
            ds_udp_connections: Vec::new(),
        }
    }

    pub fn handle_packet(&mut self, nic: &NIC, udp: &UdpHeader, ipv4: &IPv4Header, payload: &[u8]) {
        let payload_size = payload.len();

        // Lookup connection
        let mock_conn = (udp.dest_port, udp.src_port);
        let mut conn_opt: Option<Arc<Mutex<UdpConnection>>> = None;

        for conn in &self.ds_udp_connections {
            let locked = conn.lock().unwrap();
            if locked.local_port == mock_conn.0 && (locked.remote_port == 0 || locked.remote_port == mock_conn.1) {
                conn_opt = Some(conn.clone());
                break;
            }
        }

        if conn_opt.is_none() {
            println!(
                "[net::udp] Drop: No open connection! local{{{}}} remote{{{}}}",
                mock_conn.0, mock_conn.1
            );
            return;
        }

        let conn = conn_opt.unwrap();
        let mut conn_locked = conn.lock().unwrap();

        if conn_locked.total_data + payload_size > UDP_MAX_TOTAL_DATA {
            println!("[net::udp] Drop: Cannot allocate more connection buffers");
            return;
        }

        if conn_locked.ip[0] != 0 && conn_locked.ip != ipv4.src_address {
            println!("[net::udp] Drop: Sent from an invalid host");
            return;
        }

        // Store the payload
        let buffer = UdpBuffer {
            buff: payload.to_vec(),
            len: payload_size,
            remote_port: mock_conn.1,
            ip: ipv4.src_address,
        };
        conn_locked.total_data += payload_size;
        conn_locked.ds_buffers.push_back(buffer);

        // Notify poll/epoll
        poll_instance_ring(&conn);
    }
}

// Dummy poll function
fn poll_instance_ring(conn: &Arc<Mutex<UdpConnection>>) {
    // In real system, signal EPOLLIN or equivalent
    println!("[poll] EPOLLIN triggered for connection at {:p}", Arc::as_ptr(conn));
}
