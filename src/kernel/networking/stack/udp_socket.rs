use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

// --- Constants & Dummy Types ---
pub const IPv4_BYTE_SIZE: usize = 4;
pub const NET_UDP_CARRY_BARE: usize = 0; // placeholder
pub const IPV4_PROTOCOL_UDP: u8 = 17;
pub const EPOLLIN: u32 = 0x001;
pub const EPOLLOUT: u32 = 0x004;
pub const O_NONBLOCK: u32 = 0x800;

pub fn switch_endian_16(v: u16) -> u16 {
    v.to_be()
}

// Dummy NIC
pub struct NIC {
    pub udp: Arc<Mutex<UdpStore>>,
    pub mtu: usize,
}

// UDP header
pub struct UdpHeader {
    pub src_port: u16,
    pub dest_port: u16,
    pub length: u16,
    pub checksum: u16,
}

// IPv4 header
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
    pub net_ports_udp: NetPorts,
}

pub struct NetPorts; // placeholder
impl NetPorts {
    pub fn gen(&mut self) -> u16 { 10000 } // dummy ephemeral port generator
    pub fn mark_safe(&mut self, _port: u16) -> bool { true }
    pub fn free(&mut self, _port: u16) {}
}

// Userspace UDP socket
pub struct UdpSocket {
    pub conn: Option<Arc<Mutex<UdpConnection>>>,
    pub connected: bool,
    pub times_opened: usize,
}

// File descriptor abstraction
pub struct OpenFile {
    pub dir: Option<Arc<Mutex<UdpSocket>>>,
    pub flags: u32,
}

// --- UDP Socket Functions ---

pub fn net_connection_udp_open(sock: &Arc<Mutex<UdpSocket>>, nic: &Arc<Mutex<NIC>>, 
                               local_port: u16, addr: Option<[u8; 4]>, remote_port: u16) 
{
    let mut sock_lock = sock.lock().unwrap();
    let mut nic_lock = nic.lock().unwrap();
    let store = &mut nic_lock.udp.lock().unwrap();

    if sock_lock.conn.is_none() {
        let conn = Arc::new(Mutex::new(UdpConnection {
            local_port: 0,
            remote_port: 0,
            total_data: 0,
            ip: [0; IPv4_BYTE_SIZE],
            ds_buffers: VecDeque::new(),
        }));
        sock_lock.conn = Some(conn);
    }

    if let Some(ip) = addr {
        sock_lock.conn.as_ref().unwrap().lock().unwrap().ip = ip;
    }

    let local_port = if local_port != 0 {
        local_port
    } else if sock_lock.conn.as_ref().unwrap().lock().unwrap().local_port != 0 {
        sock_lock.conn.as_ref().unwrap().lock().unwrap().local_port
    } else {
        store.net_ports_udp.gen()
    };

    sock_lock.conn.as_ref().unwrap().lock().unwrap().local_port = local_port;

    if remote_port != 0 {
        sock_lock.conn.as_ref().unwrap().lock().unwrap().remote_port = remote_port;
    }

    // Debug output
    let conn_ip = sock_lock.conn.as_ref().unwrap().lock().unwrap().ip;
    if conn_ip[0] != 0 {
        println!(
            "[net::udp::sock] CONNECT TO {}.{}.{}.{}:{}",
            conn_ip[0], conn_ip[1], conn_ip[2], conn_ip[3],
            sock_lock.conn.as_ref().unwrap().lock().unwrap().remote_port
        );
    } else {
        println!(
            "[net::udp::sock] ACCEPTING TO US AT {}",
            sock_lock.conn.as_ref().unwrap().lock().unwrap().local_port
        );
    }
}

// Automatic bind for sending
pub fn net_socket_udp_sendto(fd: &Arc<Mutex<OpenFile>>, nic: &Arc<Mutex<NIC>>,
                             data: &[u8], dest_addr: Option<[u8; 4]>, dest_port: Option<u16>)
                             -> Result<usize, &'static str> 
{
    let sock = fd.lock().unwrap().dir.as_ref().unwrap().clone();
    let mut sock_lock = sock.lock().unwrap();

    if sock_lock.conn.is_none() {
        println!("[net::udp::sock] Doing the automatic bind.");
        net_connection_udp_open(&sock, nic, 0, None, 0);
    }

    let conn = sock_lock.conn.as_ref().unwrap().clone();
    let mut conn_lock = conn.lock().unwrap();

    let final_addr = dest_addr.unwrap_or(conn_lock.ip);
    let remote_port = dest_port.unwrap_or(conn_lock.remote_port);
    let local_port = conn_lock.local_port;

    if final_addr[0] == 0 {
        return Err("Destination address required");
    }

    // Packet construction
    let final_len = NET_UDP_CARRY_BARE + 8 + data.len(); // 8 = UDP header
    if final_len > nic.lock().unwrap().mtu {
        return Err("Message too large");
    }

    let mut packet = vec![0u8; final_len];
    // initialize IPv4 buffer here if needed

    // Fill UDP header
    let udp_header = UdpHeader {
        src_port: switch_endian_16(local_port),
        dest_port: switch_endian_16(remote_port),
        length: switch_endian_16(8 + data.len() as u16),
        checksum: 0,
    };
    // copy payload at end of packet
    packet[final_len - data.len()..].copy_from_slice(data);

    // Send IPv4 packet (placeholder)
    // net_ipv4_send(nic, &packet, IPV4_PROTOCOL_UDP, &final_addr);

    Ok(data.len())
}
