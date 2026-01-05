#![no_std]

use core::ptr;

use crate::{
    elf::*,
    fb::*,
    malloc::*,
    md5::*,
    ne2k::*,
    pci::*,
    shell::*,
    string::*,
    system::*,
    task::*,
    testing::*,
    util::*,
    vmm::*,
    timer::*,
    vfs::*,
    kb::*,
};

use lwip::{
    err::*,
    netdb::*,
    sockets::*,
};

extern "C" {
    fn handControl();
}

/// Equivalent of `waitNicIPAssigned`
pub fn wait_nic_ip_assigned() {
    unsafe {
        let mut pci = dsPCI.firstObject as *mut PCI;

        while !pci.is_null() {
            if (*pci).category == PCI_DRIVER_CATEGORY_NIC {
                break;
            }
            pci = (*pci)._ll.next as *mut PCI;
        }

        let nic = if !pci.is_null() {
            (*pci).extra as *mut NIC
        } else {
            ptr::null_mut()
        };

        if !nic.is_null() {
            while (*nic).lwip.ip_addr.addr == 0 {
                handControl();
            }
        }
    }
}

/// Testing init (currently empty, same as C)
pub fn testing_init() {
    // wait_nic_ip_assigned();
    // run(argv[0], true, argv.len(), argv);
}

/// Weird / experimental tests
#[no_mangle]
pub extern "C" fn weirdTests() {
    unsafe {
        let mut targ_a: u32 = 0;
        let mut targ_b: u32 = 0;
        let mut targ_c: u32 = 0;
        let mut targ_d: u32 = 0;

        let mut i = 0;
        while i < 0 {
            let dir = fsKernelOpen(
                b"/files/lorem.txt\0".as_ptr() as *const i8,
                O_RDONLY,
                0,
            );

            if dir.is_null() {
                printf(b"File cannot be found!\n\0".as_ptr() as *const i8);
                i += 1;
                continue;
            }

            let filesize = fsGetFilesize(dir);
            let out = malloc(filesize as usize) as *mut u8;

            fsRead(dir, out, filesize);
            fsKernelClose(dir);

            let ctx = malloc(core::mem::size_of::<MD5_CTX>()) as *mut MD5_CTX;
            MD5_Init(ctx);
            MD5_Update(ctx, out, filesize);

            let md = malloc(core::mem::size_of::<MD5_OUT>()) as *mut MD5_OUT;
            MD5_Final(md as *mut _, ctx);

            if i != 0
                && ((*md).a != targ_a
                    || (*md).b != targ_b
                    || (*md).c != targ_c
                    || (*md).d != targ_d)
            {
                debugf(b"FAIL! FAIL! FAIL!\n\0".as_ptr() as *const i8);
                break;
            }

            targ_a = (*md).a;
            targ_b = (*md).b;
            targ_c = (*md).c;
            targ_d = (*md).d;

            debugf(
                b"%02x%02x%02x%02x\n\0".as_ptr() as *const i8,
                switch_endian_32((*md).a),
                switch_endian_32((*md).b),
                switch_endian_32((*md).c),
                switch_endian_32((*md).d),
            );

            i += 1;
        }
    }
}
