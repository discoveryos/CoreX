
#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

//
// ===== Networking queue =====
//

struct NetPacket {
    nic: NicHandle,
    buffer: PacketBuffer,
    len: usize,
}

static NET_QUEUE: Mutex<[Option<NetPacket>; QUEUE_MAX]> =
    Mutex::new([None; QUEUE_MAX]);

static NET_READ: AtomicUsize = AtomicUsize::new(0);
static NET_WRITE: AtomicUsize = AtomicUsize::new(0);

fn helper_net() {
    loop {
        let read = NET_READ.load(Ordering::Acquire);
        let write = NET_WRITE.load(Ordering::Acquire);

        if read == write {
            return; // queue empty
        }

        let mut queue = NET_QUEUE.lock();
        if let Some(pkt) = queue[read].take() {
            NET_READ.store((read + 1) % QUEUE_MAX, Ordering::Release);
            drop(queue);

            networking::handle_packet(pkt.nic, pkt.buffer, pkt.len);
        }
    }
}

//
// ===== Reaper =====
//

static REAPER_TASK: Mutex<Option<Task>> = Mutex::new(None);

fn helper_reaper() {
    let mut guard = REAPER_TASK.lock();

    let task = match guard.as_mut() {
        Some(t) => t,
        None => return,
    };

    if task.state() == TaskState::SigKilled {
        task::kill(task.id(), 128 + task.exit_code());
    }

    if task.state() != TaskState::Dead {
        return;
    }

    // Free children
    task::free_children(task);

    // Free stacks (safe wrapper)
    memory::free_user_stack(task);

    // Free FS info
    fs::discard_task_info(task);

    // Free interrupted syscalls
    task::clear_interrupted_syscalls(task);

    // Remove task from scheduler
    task::destroy(task);

    *guard = None;
}

//
// ===== Poll helper =====
//

fn helper_poll() {
    if !console::is_disabled() {
        poll::ring(69, PollFlags::IN);
    }

    for event in dev::input_events() {
        if event.has_pending() {
            poll::ring(event.as_id(), PollFlags::IN);
        }
    }
}

//
// ===== Kernel helper entry =====
//

fn kernel_helper_loop() -> ! {
    loop {
        helper_net();
        helper_reaper();
        helper_poll();

        scheduler::yield_now();
    }
}

//
// ===== Thread creation =====
//

static NET_HELPER_TASK: Mutex<Option<TaskHandle>> = Mutex::new(None);

pub fn initiate_kernel_threads() {
    let task = task::create_kernel(kernel_helper_loop);
    task::set_name(&task, "kernel-helper");

    *NET_HELPER_TASK.lock() = Some(task);
}
