use std::sync::{Arc, Mutex};
use std::ptr;
use std::slice;
use crate::fs::*;
use crate::task::*;
use crate::syscalls::*;
use crate::util::*;
use crate::poll::*;
use crate::timer::*;
use crate::linux::*;

pub fn syscall_read(fd: usize, buf: &mut [u8]) -> Result<usize, usize> {
    if buf.is_empty() { return Ok(0); }
    let task = current_task();
    let file = fs_user_get_node(&task, fd).ok_or(EBADF)?;
    fs_read(file, buf)
}

pub fn syscall_write(fd: usize, buf: &[u8]) -> Result<usize, usize> {
    if buf.is_empty() { return Ok(0); }
    let task = current_task();
    let file = fs_user_get_node(&task, fd).ok_or(EBADF)?;
    fs_write(file, buf)
}

pub fn syscall_open(filename: &str, flags: u32, mode: u32) -> Result<usize, usize> {
    if filename.is_empty() { return Err(EFAULT); }
    let task = current_task();
    fs_user_open(&task, filename, flags, mode)
}

pub fn syscall_close(fd: usize) -> Result<usize, usize> {
    let task = current_task();
    fs_user_close(&task, fd)
}

pub fn syscall_stat(filename: &str, buf: &mut Stat) -> Result<(), usize> {
    let task = current_task();
    if fs_stat_by_filename(&task, filename, buf) { Ok(()) } else { Err(ENOENT) }
}

pub fn syscall_fstat(fd: usize, buf: &mut Stat) -> Result<(), usize> {
    let task = current_task();
    let file = fs_user_get_node(&task, fd).ok_or(EBADF)?;
    if fs_stat(file, buf) { Ok(()) } else { Err(ENOENT) }
}

pub fn syscall_lstat(filename: &str, buf: &mut Stat) -> Result<(), usize> {
    let task = current_task();
    if fs_lstat_by_filename(&task, filename, buf) { Ok(()) } else { Err(ENOENT) }
}

pub fn syscall_lseek(fd: usize, offset: isize, whence: u32) -> Result<usize, usize> {
    let task = current_task();
    fs_user_seek(&task, fd, offset, whence)
}

pub fn syscall_ioctl(fd: usize, request: u64, arg: *mut u8) -> Result<usize, usize> {
    let task = current_task();
    let file = fs_user_get_node(&task, fd).ok_or(EBADF)?;
    let handlers = file.handlers.as_ref().ok_or(ENOTTY)?;
    let _lock = file.lock_operations.lock().unwrap();
    handlers.ioctl(file, request, arg)
}

pub fn syscall_pread64(fd: usize, buf: &mut [u8], pos: usize) -> Result<usize, usize> {
    syscall_lseek(fd, pos as isize, SEEK_SET)?;
    syscall_read(fd, buf)
}

pub fn syscall_readv(fd: usize, iov: &[IoVec]) -> Result<usize, usize> {
    let task = current_task();
    let file = fs_user_get_node(&task, fd).ok_or(EBADF)?;
    let mut total = 0;
    for vec in iov {
        if vec.len == 0 { continue; }
        if total > 0 && file.handlers.as_ref().map_or(false, |h| h.internal_poll.is_some()) {
            if !file.handlers.as_ref().unwrap().internal_poll.unwrap()(file, EPOLLIN) {
                return Ok(total);
            }
        }
        match fs_read(file, unsafe { slice::from_raw_parts_mut(vec.base as *mut u8, vec.len) }) {
            Ok(n) => total += n,
            Err(e) => return if total > 0 { Ok(total) } else { Err(e) },
        }
    }
    Ok(total)
}

pub fn syscall_writev(fd: usize, iov: &[IoVec]) -> Result<usize, usize> {
    let task = current_task();
    let file = fs_user_get_node(&task, fd).ok_or(EBADF)?;
    let mut total = 0;
    for vec in iov {
        if vec.len == 0 { continue; }
        if total > 0 && file.handlers.as_ref().map_or(false, |h| h.internal_poll.is_some()) {
            if !file.handlers.as_ref().unwrap().internal_poll.unwrap()(file, EPOLLOUT) {
                return Ok(total);
            }
        }
        match fs_write(file, unsafe { slice::from_raw_parts(vec.base as *const u8, vec.len) }) {
            Ok(n) => total += n,
            Err(e) => return if total > 0 { Ok(total) } else { Err(e) },
        }
    }
    Ok(total)
}

pub fn syscall_mkdir(path: &str, mode: u32) -> Result<usize, usize> {
    let task = current_task();
    let mut fs_lock = task.info_fs.lock().unwrap();
    let mode = mode & !fs_lock.umask;
    fs_mkdir(&task, path, mode)
}

pub fn syscall_unlink(path: &str) -> Result<usize, usize> {
    let task = current_task();
    fs_unlink(&task, path, false)
}

pub fn syscall_umask(mask: u32) -> u32 {
    let task = current_task();
    let mut fs = task.info_fs.lock().unwrap();
    let old = fs.umask;
    fs.umask = mask & 0o777;
    old
}

pub fn at_resolve_pathname(dirfd: usize, pathname: &str) -> Result<String, usize> {
    if pathname.starts_with('/') || dirfd == AT_FDCWD {
        Ok(pathname.to_string())
    } else {
        let task = current_task();
        let fd = fs_user_get_node(&task, dirfd).ok_or(EBADF)?;
        let dir = fd.dirname.as_ref().ok_or(ENOTDIR)?;
        let path = format!("{}{}{}", fd.mount_point.prefix, dir, pathname);
        Ok(path)
    }
}

pub fn syscall_openat(dirfd: usize, pathname: &str, flags: u32, mode: u32) -> Result<usize, usize> {
    let resolved = at_resolve_pathname(dirfd, pathname)?;
    syscall_open(&resolved, flags, mode)
}

pub fn syscall_mkdirat(dirfd: usize, pathname: &str, mode: u32) -> Result<usize, usize> {
    let resolved = at_resolve_pathname(dirfd, pathname)?;
    syscall_mkdir(&resolved, mode)
}

// --- Registration ---
pub fn syscall_reg_fs() {
    register_syscall(SYSCALL_READ, syscall_read);
    register_syscall(SYSCALL_WRITE, syscall_write);
    register_syscall(SYSCALL_OPEN, syscall_open);
    register_syscall(SYSCALL_OPENAT, syscall_openat);
    register_syscall(SYSCALL_CLOSE, syscall_close);
    register_syscall(SYSCALL_LSEEK, syscall_lseek);
    register_syscall(SYSCALL_STAT, syscall_stat);
    register_syscall(SYSCALL_FSTAT, syscall_fstat);
    register_syscall(SYSCALL_LSTAT, syscall_lstat);
    register_syscall(SYSCALL_MKDIR, syscall_mkdir);
    register_syscall(SYSCALL_UNLINK, syscall_unlink);
    register_syscall(SYSCALL_UMASK, syscall_umask);
    register_syscall(SYSCALL_PREAD64, syscall_pread64);
    register_syscall(SYSCALL_READV, syscall_readv);
    register_syscall(SYSCALL_WRITEV, syscall_writev);
    register_syscall(SYSCALL_MKDIRAT, syscall_mkdirat);
}
