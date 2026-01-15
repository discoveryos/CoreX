#![no_std]

use core::ffi::c_void;
use core::ptr;

//
// Constants
//

const PAGE_SIZE: usize = 4096;
const USER_STACK_PAGES: usize = 8;
const USER_STACK_BOTTOM: usize = 0x0000_7fff_ffff_f000;

const PF_USER: u64 = 1 << 2;
const PF_RW: u64 = 1 << 1;

//
// Externals
//

extern "C" {
    fn debugf(fmt: *const u8, ...);
    fn panic() -> !;

    fn memset(dst: *mut c_void, val: i32, size: usize);
    fn memcpy(dst: *mut c_void, src: *const c_void, size: usize);

    fn rand() -> i32;

    fn VirtualMap(virt: usize, phys: usize, flags: u64);
    fn PhysicalAllocate(pages: i32) -> usize;

    fn GetPageDirectory() -> *mut c_void;
    fn ChangePageDirectory(pd: *mut c_void);

    fn spinlockAcquire(lock: *mut c_void);
    fn spinlockRelease(lock: *mut c_void);

    fn taskAdjustHeap(
        task: *mut Task,
        new_end: usize,
        heap_start: *mut usize,
        heap_end: *mut usize,
    );

    fn taskKill(id: u64, code: i32);
}

//
// ELF
//

#[repr(C)]
pub struct Elf64Ehdr {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
}

#[repr(C)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

const PT_LOAD: u32 = 1;

//
// Task / paging structures (partial)
//

#[repr(C)]
pub struct Registers {
    pub rdi: u64,
    pub usermode_rsp: usize,
}

#[repr(C)]
pub struct PageInfo {
    pub pagedir: *mut c_void,
    pub heap_start: usize,
    pub heap_end: usize,
    pub LOCK_PD: *mut c_void,
}

#[repr(C)]
pub struct Task {
    pub id: u64,
    pub registers: Registers,
    pub infoPd: *mut PageInfo,
}

extern "C" {
    static mut currentTask: *mut Task;
}

//
// Helpers
//

#[inline(always)]
unsafe fn strlength(s: *const u8) -> u32 {
    let mut len = 0;
    while *s.add(len as usize) != 0 {
        len += 1;
    }
    len
}

//
// Stack generation (shared)
//

pub unsafe fn stack_generate_mutual(task: *mut Task) {
    for i in 0..USER_STACK_PAGES {
        let virt_addr =
            USER_STACK_BOTTOM - USER_STACK_PAGES * PAGE_SIZE + i * PAGE_SIZE;

        VirtualMap(
            virt_addr,
            PhysicalAllocate(1),
            PF_USER | PF_RW,
        );

        memset(virt_addr as *mut c_void, 0, PAGE_SIZE);
    }
}

//
// Argument/environment storage helper
//

#[repr(C)]
pub struct StackStorePtrStyle {
    pub start: *mut u8,
    pub ellapsed: usize,
}

pub unsafe fn stack_store_ptr_style(
    target: *mut Task,
    ptrc: u32,
    ptrv: *const *const u8,
) -> StackStorePtrStyle {
    let mut arg_space: usize = 0;

    for i in 0..ptrc {
        arg_space += strlength(*ptrv.add(i as usize)) as usize + 1;
    }

    let pd = (*target).infoPd;
    spinlockAcquire((*pd).LOCK_PD);

    let arg_start = (*pd).heap_end as *mut u8;

    taskAdjustHeap(
        target,
        (*pd).heap_end + arg_space,
        &mut (*pd).heap_start,
        &mut (*pd).heap_end,
    );

    spinlockRelease((*pd).LOCK_PD);

    let mut ellapsed = 0;
    for i in 0..ptrc {
        let s = *ptrv.add(i as usize);
        let len = strlength(s) as usize + 1;
        memcpy(
            arg_start.add(ellapsed) as *mut c_void,
            s as *const c_void,
            len,
        );
        ellapsed += len;
    }

    StackStorePtrStyle {
        start: arg_start,
        ellapsed,
    }
}

//
// User stack generation
//

#[inline(always)]
unsafe fn push_to_stack<T>(rsp: &mut usize, val: T) {
    *rsp -= core::mem::size_of::<T>();
    ptr::write(*rsp as *mut T, val);
}

pub unsafe fn stack_generate_user(
    target: *mut Task,
    argc: u32,
    argv: *const *const u8,
    envc: u32,
    envv: *const *const u8,
    elf_image: *const u8,
    _filesize: usize,
    elf_ehdr_ptr: *const c_void,
    at_base: usize,
    executable_base: usize,
) {
    let elf = elf_ehdr_ptr as *const Elf64Ehdr;

    let old_pd = GetPageDirectory();
    let pd = (*target).infoPd;

    spinlockAcquire((*pd).LOCK_PD);
    ChangePageDirectory((*pd).pagedir);
    spinlockRelease((*pd).LOCK_PD);

    stack_generate_mutual(target);

    // AT_RANDOM
    spinlockAcquire((*pd).LOCK_PD);
    let random_start = (*pd).heap_end as *mut u32;
    taskAdjustHeap(
        target,
        (*pd).heap_end + 4 * core::mem::size_of::<u32>(),
        &mut (*pd).heap_start,
        &mut (*pd).heap_end,
    );
    spinlockRelease((*pd).LOCK_PD);

    for i in 0..4 {
        let mut v = 0;
        while v == 0 {
            v = rand();
        }
        *random_start.add(i) = v as u32;
    }

    let mut lowest_vaddr = 0usize;
    for i in 0..(*elf).e_phnum {
        let phdr = elf_image.add((*elf).e_phoff as usize
            + i as usize * (*elf).e_phentsize as usize)
            as *const Elf64Phdr;

        if (*phdr).p_type != PT_LOAD {
            continue;
        }

        if lowest_vaddr == 0 || (*phdr).p_vaddr as usize < lowest_vaddr {
            lowest_vaddr = (*phdr).p_vaddr as usize;
        }
    }

    let phdr_base = if (*elf).e_type == 3 { 0 } else { lowest_vaddr };

    let rsp = &mut (*target).registers.usermode_rsp;

    // AUXV (reverse order)
    push_to_stack(rsp, 0usize);
    push_to_stack(rsp, 0usize);

    push_to_stack(rsp, random_start as usize);
    push_to_stack(rsp, 25u64);

    push_to_stack(rsp, PAGE_SIZE as u64);
    push_to_stack(rsp, 6u64);

    push_to_stack(rsp, 0u64);
    push_to_stack(rsp, 23u64);

    push_to_stack(rsp, (*elf).e_phnum as u64);
    push_to_stack(rsp, 5u64);

    push_to_stack(rsp, (*elf).e_phentsize as u64);
    push_to_stack(rsp, 4u64);

    push_to_stack(rsp, executable_base + (*elf).e_entry as usize);
    push_to_stack(rsp, 9u64);

    push_to_stack(rsp, at_base as u64);
    push_to_stack(rsp, 7u64);

    push_to_stack(rsp, 0u64);
    push_to_stack(rsp, 8u64);

    push_to_stack(rsp, 0u64);
    push_to_stack(rsp, 16u64);

    push_to_stack(
        rsp,
        executable_base + phdr_base + (*elf).e_phoff as usize,
    );
    push_to_stack(rsp, 3u64);

    let args = stack_store_ptr_style(target, argc, argv);
    let envs = if envc > 0 {
        stack_store_ptr_style(target, envc, envv)
    } else {
        StackStorePtrStyle {
            start: ptr::null_mut(),
            ellapsed: 0,
        }
    };

    push_to_stack(rsp, 0u64);
    if envs.ellapsed > 0 {
        let mut used = 0;
        for i in (0..envc).rev() {
            let len = strlength(*envv.add(i as usize)) as usize + 1;
            used += len;
            push_to_stack(
                rsp,
                envs.start as usize + envs.ellapsed - used,
            );
        }
    }

    push_to_stack(rsp, 0u64);

    let mut used = 0;
    for i in (0..argc).rev() {
        let len = strlength(*argv.add(i as usize)) as usize + 1;
        used += len;
        push_to_stack(
            rsp,
            args.start as usize + args.ellapsed - used,
        );
    }

    push_to_stack(rsp, argc as u64);

    ChangePageDirectory(old_pd);
}

//
// Kernel stack
//

#[no_mangle]
pub unsafe extern "C" fn task_kernel_return() -> ! {
    taskKill((*currentTask).id, 0);
    loop {}
}

pub unsafe fn stack_generate_kernel(target: *mut Task, param: u64) {
    let old_pd = GetPageDirectory();
    let pd = (*target).infoPd;

    spinlockAcquire((*pd).LOCK_PD);
    ChangePageDirectory((*pd).pagedir);
    spinlockRelease((*pd).LOCK_PD);

    stack_generate_mutual(target);
    (*target).registers.rdi = param;

    push_to_stack(
        &mut (*target).registers.usermode_rsp,
        task_kernel_return as usize,
    );

    ChangePageDirectory(old_pd);
}
