use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// Dummy NIC and Task placeholders
pub struct NIC;
pub struct Task;

// Queue component type
#[derive(Clone, Copy)]
pub enum NetQueueComp {
    ARP,
    UDP,
    TCP,
    // extend as needed
}

// Queue item
pub struct NetQueueItem {
    pub component: NetQueueComp,
    pub unix_time: u64,
    pub task: Arc<Task>,
    pub nic: Arc<NIC>,
    pub callback: Option<Box<dyn Fn(&mut NetQueueItem) + Send + Sync>>,
    // Optional user data
    pub dir: Option<Box<[u8]>>,
}

// Global queue
lazy_static::lazy_static! {
    static ref DS_NET_QUEUE: Mutex<Vec<Arc<Mutex<NetQueueItem>>>> = Mutex::new(Vec::new());
}

// Current task placeholder (in real system, replace with actual task handle)
lazy_static::lazy_static! {
    static ref CURRENT_TASK: Arc<Task> = Arc::new(Task {});
}

// Get current UNIX time in milliseconds
fn current_unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// The main callback for each item
fn net_queue_main_cb(item: &Arc<Mutex<NetQueueItem>>) {
    let mut locked_item = item.lock().unwrap();
    if let Some(cb) = &locked_item.callback {
        cb(&mut locked_item);
    }
}

// Periodically called helper
pub fn helper_net_queue() {
    let queue = DS_NET_QUEUE.lock().unwrap();
    for item in queue.iter() {
        net_queue_main_cb(item);
    }
}

// Allocate a new queue item
pub fn net_queue_allocate(
    component: NetQueueComp,
    nic: Arc<NIC>,
    callback: Option<Box<dyn Fn(&mut NetQueueItem) + Send + Sync>>,
) -> Arc<Mutex<NetQueueItem>> {
    let item = Arc::new(Mutex::new(NetQueueItem {
        component,
        unix_time: current_unix_time(),
        task: CURRENT_TASK.clone(),
        nic,
        callback,
        dir: None,
    }));

    let mut queue = DS_NET_QUEUE.lock().unwrap();
    queue.push(item.clone());
    item
}
