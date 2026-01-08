use alloc::vec::Vec;
use alloc::string::String;
use core::sync::atomic::{AtomicUsize, Ordering};

struct UserspaceProc {
    pid: u64,
    times_opened: AtomicUsize,
}

impl UserspaceProc {
    fn new(pid: u64) -> Self {
        Self {
            pid,
            times_opened: AtomicUsize::new(1),
        }
    }

    fn inc(&self) {
        self.times_opened.fetch_add(1, Ordering::SeqCst);
    }

    fn dec(&self) -> bool {
        self.times_opened.fetch_sub(1, Ordering::SeqCst) == 1
    }
}

// Example: /proc/meminfo
fn meminfo_read(fd_pointer: usize, buf: &mut [u8]) -> usize {
    let total = 8 * 1024; // kB
    let free = 4 * 1024;
    let cached = 1 * 1024;
    let available = free + cached;

    let content = format!(
        "{:<15} {:>10} kB\n{:<15} {:>10} kB\n{:<15} {:>10} kB\n{:<15} {:>10} kB\n",
        "MemTotal:", total,
        "MemFree:", free,
        "MemAvailable:", available,
        "Cached:", cached
    );

    let content_bytes = content.as_bytes();
    let start = fd_pointer;
    let len = core::cmp::min(buf.len(), content_bytes.len().saturating_sub(start));
    buf[..len].copy_from_slice(&content_bytes[start..start + len]);
    len
}

// /proc/[pid]/cmdline
fn proc_cmdline_read(proc: &UserspaceProc, fd_pointer: usize, buf: &mut [u8]) -> usize {
    let task = task_get(proc.pid).expect("task not found"); // Kernel function
    let cmdline_bytes = task.cmdline.as_bytes();
    let start = fd_pointer;
    let len = core::cmp::min(buf.len(), cmdline_bytes.len().saturating_sub(start));
    buf[..len].copy_from_slice(&cmdline_bytes[start..start + len]);
    len
}

// Proc directory listing (getdents64)
fn proc_getdents64(fd_pointer: usize, buf: &mut Vec<String>) {
    for task in all_tasks() {
        if task.id == task.tgid && task.state != TaskState::Dead && task.tgid > fd_pointer as u32 {
            buf.push(task.tgid.to_string());
        }
    }
}

// Open/Close/Duplicate
fn proc_each_open(filename: &str) -> UserspaceProc {
    // parse PID from path
    let pid = if filename == "self" { current_task().id } else {
        filename[1..].parse::<u64>().unwrap_or(0)
    };
    UserspaceProc::new(pid)
}

fn proc_each_duplicate(uproc: &UserspaceProc) {
    uproc.inc();
}

fn proc_each_close(uproc: UserspaceProc) {
    if uproc.dec() {
        drop(uproc);
    }
}

// Mounting procfs pseudo-filesystem
fn proc_mount(mount_point: &mut MountPoint) {
    mount_point.handlers = &FAKEFS_HANDLERS;
    mount_point.fs_info = Box::new(FakefsOverlay::new());
    proc_setup();
}

fn proc_setup() {
    // Register /proc/meminfo, /proc/uptime, etc.
    add_file("/proc/meminfo", meminfo_read);
    add_file("/proc/uptime", uptime_read);
    add_file("/proc/stat", stat_read);
    add_dir("/proc/*", proc_root_handlers);
    add_dir("/proc/self", proc_root_handlers);
}
