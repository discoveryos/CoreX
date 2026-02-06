use core::ptr::null_mut;
use core::mem::size_of;

use crate::paging::*;
use crate::task::*;
use crate::util::*;
use crate::fs::*;

// Constants
const PAGE_SIZE: usize = 0x1000;

bitflags::bitflags! {
    pub struct MmapFlags: u32 {
        const MAP_FIXED     = 0x10;
        const MAP_ANONYMOUS = 0x20;
        const MAP_SHARED    = 0x01;
        const MAP_PRIVATE   = 0x02;
    }
}

bitflags::bitflags! {
    pub struct ProtFlags: u32 {
        const PROT_READ  = 0x1;
        const PROT_WRITE = 0x2;
        const PROT_EXEC  = 0x4;
    }
}

// ==========================
// Syscall: mmap
// ==========================
pub fn syscall_mmap(
    task: &mut Task,
    addr: usize,
    length: usize,
    prot: ProtFlags,
    flags: MmapFlags,
    fd: i32,
    pgoffset: usize,
) -> Result<usize, i32> {
    if length == 0 || (addr != 0 && addr % PAGE_SIZE != 0) {
        return Err(EINVAL);
    }

    let length_aligned = ((length + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;

    unsafe {
        if flags.contains(MmapFlags::MAP_FIXED) && flags.contains(MmapFlags::MAP_ANONYMOUS) {
            let pages = length_aligned / PAGE_SIZE;
            let end = addr + pages * PAGE_SIZE;

            task.info_pd.lock();
            if end > task.info_pd.mmap_end {
                task.info_pd.mmap_end = end;
            }
            task.info_pd.unlock();

            for i in 0..pages {
                let phys = physical_allocate(1)?;
                virtual_map(addr + i * PAGE_SIZE, phys, PF_RW | PF_USER);
            }

            core::ptr::write_bytes(addr as *mut u8, 0, pages * PAGE_SIZE);
            return Ok(addr);
        }

        // Heap allocation for anonymous memory
        if fd == -1 && !flags.contains(MmapFlags::MAP_FIXED) && flags.contains(MmapFlags::MAP_ANONYMOUS) {
            task.info_pd.lock();
            let curr = task.info_pd.mmap_end;
            task_adjust_heap(task, task.info_pd.mmap_end + length_aligned);
            task.info_pd.unlock();

            core::ptr::write_bytes(curr as *mut u8, 0, length_aligned);
            return Ok(curr);
        }

        // File-backed mmap
        if fd != -1 {
            let file = task.get_file(fd).ok_or(EBADF)?;
            if let Some(handler) = file.handlers.mmap {
                file.lock_operations();
                let res = handler(addr, length_aligned, prot, flags, file, pgoffset);
                file.unlock_operations();
                return Ok(res);
            } else {
                return Err(ENOSYS);
            }
        }
    }

    Err(ENOSYS)
}

// ==========================
// Syscall: munmap
// ==========================
pub fn syscall_munmap(task: &mut Task, addr: usize, len: usize) -> Result<(), i32> {
    if addr % PAGE_SIZE != 0 || len == 0 {
        return Err(EINVAL);
    }

    task.info_pd.lock();
    let inside_bounds = addr >= task.info_pd.mmap_start && (addr + len) <= task.info_pd.mmap_end;
    task.info_pd.unlock();

    if !inside_bounds {
        return Err(EINVAL);
    }

    let pages = (len + PAGE_SIZE - 1) / PAGE_SIZE;

    for i in 0..pages {
        unsafe {
            let phys = virtual_to_physical(addr + i * PAGE_SIZE);
            if phys != 0 {
                virtual_map(addr + i * PAGE_SIZE, 0, PF_USER);
            }
        }
    }

    Ok(())
}

// ==========================
// Syscall: brk
// ==========================
pub fn syscall_brk(task: &mut Task, brk: usize) -> Result<usize, i32> {
    task.info_pd.lock();

    if brk == 0 {
        let ret = task.info_pd.heap_end;
        task.info_pd.unlock();
        return Ok(ret);
    }

    if brk < task.info_pd.heap_end {
        task.info_pd.unlock();
        return Err(EINVAL); // shrinking not supported yet
    }

    task_adjust_heap(task, brk);

    let ret = task.info_pd.heap_end;
    task.info_pd.unlock();
    Ok(ret)
}

// ==========================
// Syscall: mprotect (stub)
// ==========================
pub fn syscall_mprotect(_start: usize, _len: usize, _prot: ProtFlags) -> Result<(), i32> {
    // TODO: Implement page permissions change
    Ok(())
}

// ==========================
// Register memory syscalls
// ==========================
pub fn syscall_reg_mem() {
    register_syscall(SYSCALL_MMAP, syscall_mmap as usize);
    register_syscall(SYSCALL_MUNMAP, syscall_munmap as usize);
    register_syscall(SYSCALL_MPROTECT, syscall_mprotect as usize);
    register_syscall(SYSCALL_BRK, syscall_brk as usize);
}
