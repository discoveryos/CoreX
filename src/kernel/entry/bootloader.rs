

use core::ptr;

use crate::system::{bootloader, debugf, panic};
use crate::limine::*;

// Parses information off our bootloader (limine)
// Copyright (C) 2025 kevin dan mathew

#[used]
#[link_section = ".limine_reqs"]
static LIMINE_PAGING_REQ: limine_paging_mode_request = limine_paging_mode_request {
    id: LIMINE_PAGING_MODE_REQUEST,
    revision: 0,
    response: ptr::null_mut(),
    mode: LIMINE_PAGING_MODE_X86_64_4LVL,
};

#[used]
#[link_section = ".limine_reqs"]
static LIMINE_KERNEL_REQ: limine_kernel_address_request = limine_kernel_address_request {
    id: LIMINE_KERNEL_ADDRESS_REQUEST,
    revision: 0,
    response: ptr::null_mut(),
};

#[used]
#[link_section = ".limine_reqs"]
static LIMINE_HHDM_REQ: limine_hhdm_request = limine_hhdm_request {
    id: LIMINE_HHDM_REQUEST,
    revision: 0,
    response: ptr::null_mut(),
};

#[used]
#[link_section = ".limine_reqs"]
static LIMINE_MMAP_REQ: limine_memmap_request = limine_memmap_request {
    id: LIMINE_MEMMAP_REQUEST,
    revision: 0,
    response: ptr::null_mut(),
};

#[used]
#[link_section = ".limine_reqs"]
static LIMINE_SMP_REQ: limine_smp_request = limine_smp_request {
    id: LIMINE_SMP_REQUEST,
    revision: 0,
    response: ptr::null_mut(),
};

#[used]
#[link_section = ".limine_reqs"]
static LIMINE_RSDP_REQ: limine_rsdp_request = limine_rsdp_request {
    id: LIMINE_RSDP_REQUEST,
    revision: 0,
    response: ptr::null_mut(),
};

pub unsafe fn initialise_bootloader_parser() {
    // Paging mode
    let paging_res = LIMINE_PAGING_REQ.response.as_ref().unwrap_or_else(|| {
        debugf("[paging] Missing paging mode response\n");
        panic();
    });

    if paging_res.mode != LIMINE_PAGING_MODE_X86_64_4LVL {
        debugf("[paging] We explicitly asked for level 4 paging!\n");
        panic();
    }

    // HHDM
    let hhdm_res = LIMINE_HHDM_REQ.response.as_ref().unwrap_or_else(|| {
        debugf("[bootloader] Missing HHDM response\n");
        panic();
    });

    bootloader.hhdm_offset = hhdm_res.offset;

    // Kernel address
    let kernel_res = LIMINE_KERNEL_REQ.response.as_ref().unwrap_or_else(|| {
        debugf("[bootloader] Missing kernel address response\n");
        panic();
    });

    bootloader.kernel_virt_base = kernel_res.virtual_base;
    bootloader.kernel_phys_base = kernel_res.physical_base;

    // Memory map
    let mmap_res = LIMINE_MMAP_REQ.response.as_ref().unwrap_or_else(|| {
        debugf("[bootloader] Missing memory map response\n");
        panic();
    });

    bootloader.mm_entries = mmap_res.entries;
    bootloader.mm_entry_cnt = mmap_res.entry_count;

    // Total usable memory
    bootloader.mm_total = 0;

    for i in 0..mmap_res.entry_count {
        let entry = &*mmap_res.entries.add(i as usize);

        if entry.typ == LIMINE_MEMMAP_USABLE {
            bootloader.mm_total += entry.length;
        }
    }

    // SMP
    let smp_res = LIMINE_SMP_REQ.response.as_ref().unwrap_or_else(|| {
        debugf("[bootloader] Missing SMP response\n");
        panic();
    });

    bootloader.smp = smp_res as *const _;

    // Bootstrap core index
    bootloader.smp_bsp_index = u64::MAX;

    for i in 0..smp_res.cpu_count {
        let cpu = &*smp_res.cpus.add(i as usize);
        if cpu.lapic_id == smp_res.bsp_lapic_id {
            bootloader.smp_bsp_index = i;
            break;
        }
    }

    if bootloader.smp_bsp_index == u64::MAX {
        debugf("[bootloader] Couldn't find bootstrap core!\n");
        panic();
    }

    // RSDP
    let rsdp_res = LIMINE_RSDP_REQ.response.as_ref().unwrap_or_else(|| {
        debugf("[bootloader] Missing RSDP response\n");
        panic();
    });

    if rsdp_res.revision < 3 {
        debugf("[bootloader] RSDP revision too old\n");
        panic();
    }

    bootloader.rsdp =
        (rsdp_res.address as usize).wrapping_sub(bootloader.hhdm_offset as usize);
}
