#![no_std]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::{mem, ptr, slice};

type bool_t = bool;
type size_t = usize;
type uint8_t = u8;
type uint32_t = u32;
type uint64_t = u64;
type int32_t = i32;

/* ================= ELF CONSTANTS ================= */

const EI_MAG0: usize = 0;
const EI_MAG1: usize = 1;
const EI_MAG2: usize = 2;
const EI_MAG3: usize = 3;
const EI_CLASS: usize = 4;

const ELFMAG0: u8 = 0x7f;
const ELFMAG1: u8 = b'E';
const ELFMAG2: u8 = b'L';
const ELFMAG3: u8 = b'F';

const ELFCLASS64: u8 = 2;
const ELF_x86_64_MACHINE: u16 = 62;

const PT_LOAD: u32 = 1;
const PT_INTERP: u32 = 3;
const ET_DYN: u16 = 3;

const PF_USER: u64 = 1 << 2;
const PF_RW: u64 = 1 << 1;

const BLOCK_SIZE: usize = 512;

/* ================= ELF STRUCTS ================= */

#[repr(C)]
pub struct Elf64_Ehdr {
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
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
pub struct Elf64_Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

/* ================= KERNEL EXTERNS ================= */

#[repr(C)]
pub struct OpenFile;
#[repr(C)]
pub struct Task;
#[repr(C)]
pub struct PageDirectory;

extern "C" {
    fn debugf(fmt: *const u8, ...) ;
    fn panic() -> !;

    fn fsKernelOpen(path: *const u8, flags: u32, mode: u32) -> *mut OpenFile;
    fn fsGetFilesize(file: *mut OpenFile) -> size_t;
    fn fsRead(file: *mut OpenFile, buf: *mut u8, size: size_t);
    fn fsKernelClose(file: *mut OpenFile);

    fn VirtualAllocate(pages: size_t) -> *mut u8;
    fn VirtualFree(ptr: *mut u8, pages: size_t);

    fn PhysicalAllocate(pages: size_t) -> size_t;
    fn VirtualMap(vaddr: size_t, paddr: size_t, flags: u64);
    fn VirtualToPhysical(vaddr: size_t) -> size_t;

    fn GetPageDirectory() -> *mut PageDirectory;
    fn PageDirectoryAllocate() -> *mut PageDirectory;
    fn ChangePageDirectory(pd: *mut PageDirectory);

    fn taskGenerateId() -> int32_t;
    fn taskCreate(
        id: int32_t,
        entry: size_t,
        kernel: bool,
        pagedir: *mut PageDirectory,
        argc: u32,
        argv: *mut *mut u8,
    ) -> *mut Task;

    fn taskCreateFinish(task: *mut Task);
    fn stackGenerateUser(
        task: *mut Task,
        argc: u32,
        argv: *mut *mut u8,
        envc: u32,
        envv: *mut *mut u8,
        image: *mut u8,
        size: size_t,
        ehdr: *mut Elf64_Ehdr,
        interp_base: size_t,
        exec_base: size_t,
    );

    fn DivRoundUp(a: size_t, b: size_t) -> size_t;
    fn strlength(s: *const u8) -> size_t;
    fn strdup(s: *const u8) -> *mut u8;
    fn strEql(a: *const u8, b: *const u8) -> bool;
}

/* ================= ELF FUNCTIONS ================= */

pub unsafe fn elf_check_file(hdr: *const Elf64_Ehdr) -> bool {
    if hdr.is_null() {
        return false;
    }

    let h = &*hdr;

    if h.e_ident[EI_MAG0] != ELFMAG0 ||
       h.e_ident[EI_MAG1] != ELFMAG1 ||
       h.e_ident[EI_MAG2] != ELFMAG2 ||
       h.e_ident[EI_MAG3] != ELFMAG3 {
        debugf(b"[elf] Invalid ELF magic\n\0".as_ptr());
        return false;
    }

    if h.e_ident[EI_CLASS] != ELFCLASS64 || h.e_machine != ELF_x86_64_MACHINE {
        debugf(b"[elf] Unsupported architecture\n\0".as_ptr());
        return false;
    }

    true
}

unsafe fn elf_process_load(phdr: &Elf64_Phdr, image: *mut u8, base: size_t) {
    let start = (phdr.p_vaddr as size_t) & !0xFFF;
    let pages = DivRoundUp(
        (phdr.p_vaddr as size_t - start) + phdr.p_memsz as size_t,
        0x1000,
    );

    for i in 0..pages {
        let vaddr = start + i * 0x1000;
        if VirtualToPhysical(base + vaddr) != 0 {
            continue;
        }
        let paddr = PhysicalAllocate(1);
        VirtualMap(base + vaddr, paddr, PF_USER | PF_RW);
    }

    ptr::copy_nonoverlapping(
        image.add(phdr.p_offset as usize),
        (base + phdr.p_vaddr as usize) as *mut u8,
        phdr.p_filesz as usize,
    );

    if phdr.p_memsz > phdr.p_filesz {
        ptr::write_bytes(
            (base + phdr.p_vaddr as usize + phdr.p_filesz as usize) as *mut u8,
            0,
            (phdr.p_memsz - phdr.p_filesz) as usize,
        );
    }
}

/* ================= ELF EXEC ================= */

pub unsafe fn elfExecute(
    filepath: *const u8,
    argc: u32,
    argv: *mut *mut u8,
    envc: u32,
    envv: *mut *mut u8,
    startup: bool,
) -> *mut Task {
    let file = fsKernelOpen(filepath, 0, 0);
    if file.is_null() {
        debugf(b"[elf] Could not open file\n\0".as_ptr());
        return ptr::null_mut();
    }

    let filesize = fsGetFilesize(file);
    let pages = DivRoundUp(filesize, BLOCK_SIZE);
    let image = VirtualAllocate(pages);
    fsRead(file, image, filesize);
    fsKernelClose(file);

    let ehdr = &*(image as *const Elf64_Ehdr);
    if !elf_check_file(ehdr) {
        return ptr::null_mut();
    }

    let old_pd = GetPageDirectory();
    let new_pd = PageDirectoryAllocate();
    ChangePageDirectory(new_pd);

    let id = taskGenerateId();
    if id == -1 {
        panic();
    }

    let mut interp_entry = 0usize;
    let interp_base = 0x1000_0000_0000usize;
    let mut exec_base = 0usize;

    for i in 0..ehdr.e_phnum {
        let phdr = &*((image as usize
            + ehdr.e_phoff as usize
            + i as usize * ehdr.e_phentsize as usize) as *const Elf64_Phdr);

        if phdr.p_type == PT_INTERP {
            let path = image.add(phdr.p_offset as usize);
            let interp = fsKernelOpen(path, 0, 0);
            if interp.is_null() {
                panic();
            }

            let size = fsGetFilesize(interp);
            let pages = DivRoundUp(size, BLOCK_SIZE);
            let buf = VirtualAllocate(pages);
            fsRead(interp, buf, size);
            fsKernelClose(interp);

            let ihdr = &*(buf as *const Elf64_Ehdr);
            interp_entry = ihdr.e_entry as usize;

            for j in 0..ihdr.e_phnum {
                let iph =
                    &*((buf as usize + ihdr.e_phoff as usize
                        + j as usize * ihdr.e_phentsize as usize)
                        as *const Elf64_Phdr);
                if iph.p_type == PT_LOAD {
                    elf_process_load(iph, buf, interp_base);
                }
            }

            VirtualFree(buf, pages);
            continue;
        }

        if phdr.p_type != PT_LOAD {
            continue;
        }

        if ehdr.e_type == ET_DYN {
            exec_base = 0x5000_0000_000usize;
        }

        elf_process_load(phdr, image, exec_base);
    }

    ChangePageDirectory(old_pd);

    let entry = if interp_entry != 0 {
        interp_base + interp_entry
    } else {
        exec_base + ehdr.e_entry as usize
    };

    let task = taskCreate(id, entry, false, new_pd, argc, argv);

    stackGenerateUser(
        task,
        argc,
        argv,
        envc,
        envv,
        image,
        filesize,
        ehdr as *const _ as *mut _,
        if interp_entry != 0 { interp_base } else { 0 },
        exec_base,
    );

    VirtualFree(image, pages);

    if startup {
        taskCreateFinish(task);
    }

    task
}
