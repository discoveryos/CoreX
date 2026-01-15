use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

// Constants (adapt to your stack)
const NETPORTS_EPH_START: u16 = 1024;
const NETPORTS_AVAILABLE_PORTS: u16 = 65535;

// Bitmap abstraction
#[derive(Default)]
struct Bitmap {
    map: Vec<AtomicBool>, // Each index represents a port
}

impl Bitmap {
    fn new(size: usize) -> Self {
        Self {
            map: (0..size).map(|_| AtomicBool::new(false)).collect(),
        }
    }

    fn get(&self, idx: usize) -> bool {
        self.map[idx].load(Ordering::SeqCst)
    }

    fn set(&self, idx: usize, val: bool) {
        self.map[idx].store(val, Ordering::SeqCst);
    }
}

// NetPorts structure
pub struct NetPorts {
    ports: Bitmap,
    lock: Mutex<()>,
    last_eph_port: u16,
}

impl NetPorts {
    pub fn new() -> Self {
        Self {
            ports: Bitmap::new(NETPORTS_AVAILABLE_PORTS as usize),
            lock: Mutex::new(()),
            last_eph_port: NETPORTS_EPH_START + rand::random::<u16>() % 128,
        }
    }

    // Check if a port is allocated
    pub fn is_allocated(&self, port: u16) -> bool {
        if port < 8 {
            return false;
        }
        let _guard = self.lock.lock().unwrap();
        self.ports.get(port as usize)
    }

    // Free a port
    pub fn free(&mut self, port: u16) {
        assert!(port != 0);
        let _guard = self.lock.lock().unwrap();
        assert!(self.ports.get(port as usize));
        self.ports.set(port as usize, false);
    }

    // Internal allocation helper
    fn gen_inner(&mut self, port: u16) -> u16 {
        self.ports.set(port as usize, true);
        self.last_eph_port = port;
        port
    }

    // Generate a new ephemeral port
    pub fn gen(&mut self) -> u16 {
        let _guard = self.lock.lock().unwrap();

        self.last_eph_port = self.last_eph_port.wrapping_add(1);
        if self.last_eph_port < NETPORTS_EPH_START {
            self.last_eph_port = NETPORTS_EPH_START + rand::random::<u16>() % 128;
        }

        // Try higher ports first
        for i in self.last_eph_port..NETPORTS_AVAILABLE_PORTS {
            if !self.ports.get(i as usize) {
                return self.gen_inner(i);
            }
        }

        // Wrap around and try lower ports
        for i in NETPORTS_EPH_START..self.last_eph_port {
            if !self.ports.get(i as usize) {
                return self.gen_inner(i);
            }
        }

        panic!("Too many connections, no free ephemeral ports left");
    }

    // Mark a port as allocated safely
    // Returns false if it was already allocated
    pub fn mark_safe(&mut self, port: u16) -> bool {
        let _guard = self.lock.lock().unwrap();
        let existing = self.ports.get(port as usize);
        if !existing {
            self.ports.set(port as usize, true);
        }
        !existing
    }
}
