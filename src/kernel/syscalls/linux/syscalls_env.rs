use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::ptr;

use crate::task::*;
use crate::fs::*;
use crate::syscalls::*;
use crate::util::*;

/// OS system information
pub struct UtsName {
    pub sysname: [u8; 65],
    pub nodename: [u8; 65],
    pub release: [u8; 65],
    pub version: [u8; 65],
    pub machine: [u8; 65],
}

pub struct RLimit {
    pub rlim_cur: usize,
    pub rlim_max: usize,
}

/// Kernel representation of the current task
/// This would be set per-core or per-thread
static mut CURRENT_TASK: Option<Arc<Mutex<Task>>> = None;

/// Helper to get the current task safely
fn current_task() -> Arc<Mutex<Task>> {
    unsafe { CURRENT_TASK.as_ref().unwrap().clone() }
}

/// ==============================
/// Syscalls Implementation
/// ==============================

pub fn syscall_getpid() -> usize {
    let task = current_task();
    let task = task.lock().unwrap();
    task.tgid
}

pub fn syscall_gettid() -> usize {
    let task = current_task();
    let task = task.lock().unwrap();
    task.id
}

pub fn syscall_getppid() -> usize {
    let task = current_task();
    let task = task.lock().unwrap();
    task.parent.as_ref().map_or(KERNEL_TASK_ID, |p| p.id)
}

pub fn syscall_getpgid() -> usize {
    let task = current_task();
    let task = task.lock().unwrap();
    task.pgid
}

pub fn syscall_setpgid(pid: Option<usize>, pgid: usize) -> Result<(), usize> {
    let task = current_task();
    let mut task = task.lock().unwrap();

    let target_pid = pid.unwrap_or(task.id);
    let target_task = task_get(target_pid).ok_or(EPERM)?;
    let mut target_task = target_task.lock().unwrap();

    target_task.pgid = pgid;
    Ok(())
}

pub fn syscall_getcwd(buf: &mut [u8]) -> Result<usize, usize> {
    let task = current_task();
    let task = task.lock().unwrap();

    let cwd_bytes = task.info_fs.cwd.as_bytes();
    let len = cwd_bytes.len() + 1;

    if buf.len() < len {
        return Err(ERANGE);
    }

    buf[..cwd_bytes.len()].copy_from_slice(cwd_bytes);
    buf[cwd_bytes.len()] = 0;
    Ok(len)
}

pub fn syscall_chdir(new_dir: &str) -> Result<(), usize> {
    let task = current_task();
    let mut task = task.lock().unwrap();

    task_change_cwd(&mut task, new_dir)
}

pub fn syscall_fchdir(fd: usize) -> Result<(), usize> {
    let task = current_task();
    let task = task.lock().unwrap();

    let file = fs_user_get_node(&task, fd).ok_or(EBADF)?;
    let dirname = file.dirname.as_ref().ok_or(ENOTDIR)?;
    drop(task); // release lock before calling chdir
    syscall_chdir(dirname)
}

pub fn syscall_getrlimit(resource: usize) -> Result<RLimit, usize> {
    match resource {
        7 => {
            let task = current_task();
            let task = task.lock().unwrap();
            Ok(RLimit {
                rlim_cur: task.info_files.rlimit_fds_soft,
                rlim_max: task.info_files.rlimit_fds_hard,
            })
        }
        _ => Err(ENOSYS),
    }
}

pub fn syscall_getuid() -> usize { 0 }
pub fn syscall_geteuid() -> usize { 0 }
pub fn syscall_getgid() -> usize { 0 }
pub fn syscall_getegid() -> usize { 0 }

pub fn syscall_setuid(uid: usize) -> Result<(), usize> {
    if uid != 0 { Err(EPERM) } else { Ok(()) }
}
pub fn syscall_setgid(gid: usize) -> Result<(), usize> {
    if gid != 0 { Err(EPERM) } else { Ok(()) }
}

pub fn syscall_uname(uts: &mut UtsName) {
    macro_rules! copy_str {
        ($dst:expr, $src:expr) => {{
            let bytes = $src.as_bytes();
            for i in 0..bytes.len() {
                $dst[i] = bytes[i];
            }
        }};
    }

    copy_str!(uts.sysname, "Cave-Like Operating System");
    copy_str!(uts.nodename, "cavOS");
    copy_str!(uts.release, "0.69.2");
    copy_str!(uts.version, "0.69.2");
    copy_str!(uts.machine, "x86_64");
}

pub fn syscall_getgroups(gidsetsize: usize, gids: &mut [u32]) -> usize {
    if gidsetsize == 0 {
        1
    } else {
        gids[0] = 0;
        1
    }
}

pub fn syscall_prctl(code: u32, addr: usize) -> Result<(), usize> {
    match code {
        0x1002 => {
            let task = current_task();
            let mut task = task.lock().unwrap();
            task.fsbase = addr;
            wrmsr(MSRID_FSBASE, task.fsbase);
            Ok(())
        }
        _ => Err(ENOSYS),
    }
}

pub fn syscall_set_tid_address(tidptr: *mut i32) -> usize {
    let task = current_task();
    let mut task = task.lock().unwrap();
    task.tidptr = tidptr;
    task.id
}

pub fn syscall_getrandom(buf: &mut [u8]) -> usize {
    let mut rng_seed = timer_ticks();
    for b in buf.iter_mut() {
        rng_seed ^= rng_seed >> 12;
        rng_seed ^= rng_seed << 25;
        rng_seed ^= rng_seed >> 27;
        *b = (rng_seed & 0xFF) as u8;
    }
    buf.len()
}

/// ==============================
/// Register Syscalls
/// ==============================
pub fn syscalls_reg_env() {
    register_syscall(SYSCALL_GETPID, syscall_getpid);
    register_syscall(SYSCALL_GETCWD, syscall_getcwd);
    register_syscall(SYSCALL_CHDIR, syscall_chdir);
    register_syscall(SYSCALL_GETRLIMIT, syscall_getrlimit);
    register_syscall(SYSCALL_GETUID, syscall_getuid);
    register_syscall(SYSCALL_GETEUID, syscall_geteuid);
    register_syscall(SYSCALL_GETGID, syscall_getgid);
    register_syscall(SYSCALL_GETEGID, syscall_getegid);
    register_syscall(SYSCALL_GETPPID, syscall_getppid);
    register_syscall(SYSCALL_GETPGID, syscall_getpgid);
    register_syscall(SYSCALL_SETPGID, syscall_setpgid);
    register_syscall(SYSCALL_SETUID, syscall_setuid);
    register_syscall(SYSCALL_SETGID, syscall_setgid);
    register_syscall(SYSCALL_SETSID, syscall_setsid);
    register_syscall(SYSCALL_PRCTL, syscall_prctl);
    register_syscall(SYSCALL_SET_TID_ADDR, syscall_set_tid_address);
    register_syscall(SYSCALL_GET_TID, syscall_gettid);
    register_syscall(SYSCALL_UNAME, syscall_uname);
    register_syscall(SYSCALL_FCHDIR, syscall_fchdir);
    register_syscall(SYSCALL_GETGROUPS, syscall_getgroups);
    register_syscall(SYSCALL_GETRANDOM, syscall_getrandom);
}
