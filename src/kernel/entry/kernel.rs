
#![no_std]
#![no_main]

use core::panic::PanicInfo;

// Limine base revision
#[used]
#[link_section = ".limine_reqs"]
static LIMINE_BASE_REVISION: limine::BaseRevision = limine::BaseRevision::new(2);

// External symbols (C kernel subsystems)
extern "C" {
    static mut systemDiskInit: bool;

    fn panic();
    fn initiateSerial();
    fn initialiseBootloaderParser();

    fn initiateVGA();
    fn initiateConsole();
    fn clearScreen();

    fn initiatePMM();
    fn initiateVMM();

    fn initiateGDT();
    fn initiateACPI();
    fn initiateISR();
    fn initiatePaging();

    fn initiateApicTimer();

    fn initiateKb();
    fn initiateMouse();

    fn initiateTasks();
    fn initiateKernelThreads();

    fn initiateNetworking();
    fn initiatePCI();

    fn fsMount(
        path: *const u8,
        connector: u32,
        a: u64,
        b: u64,
    );

    fn psfLoadFromFile(path: *const u8);

    fn initiateSyscallInst();
    fn initiateSyscalls();
    fn initiateSSE();

    fn testingInit();

    fn printf(fmt: *const u8, ...);
    fn run(path: *const u8, wait: bool, argc: u64, argv: u64);
}

// Constants (match your C headers)
const CONNECTOR_DEV: u32 = 0;
const CONNECTOR_AHCI: u32 = 1;
const CONNECTOR_SYS: u32 = 2;
const CONNECTOR_PROC: u32 = 3;

const DEFAULT_FONT_PATH: *const u8 = b"/sys/font.psf\0";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        systemDiskInit = false;

        if !limine::LIMINE_BASE_REVISION_SUPPORTED {
            panic();
        }

        initiateSerial();
        initialiseBootloaderParser();

        initiateVGA();
        initiateConsole();
        clearScreen();

        initiatePMM();
        initiateVMM();

        initiateGDT();
        initiateACPI();
        initiateISR();
        initiatePaging();

        initiateApicTimer();

        fsMount(b"/dev/\0".as_ptr(), CONNECTOR_DEV, 0, 0);
        initiateKb();
        initiateMouse();

        initiateTasks();
        initiateKernelThreads();

        initiateNetworking();
        initiatePCI();

        fsMount(b"/\0".as_ptr(), CONNECTOR_AHCI, 0, 1);
        fsMount(b"/boot/\0".as_ptr(), CONNECTOR_AHCI, 0, 0);
        fsMount(b"/sys/\0".as_ptr(), CONNECTOR_SYS, 0, 0);
        fsMount(b"/proc/\0".as_ptr(), CONNECTOR_PROC, 0, 0);

        psfLoadFromFile(DEFAULT_FONT_PATH);

        initiateSyscallInst();
        initiateSyscalls();
        initiateSSE();

        testingInit();

        if !systemDiskInit {
            printf(b"[warning] System disk has not been detected!\n\0".as_ptr());
        }

        printf(b"=========================================\n\0".as_ptr());
        printf(b"==     Cave-Like Operating System      ==\n\0".as_ptr());
        printf(b"==      Copyright MalwarePad 2025      ==\n\0".as_ptr());
        printf(b"=========================================\n\n\0".as_ptr());

        loop {
            run(b"/bin/bash\0".as_ptr(), true, 0, 0);
        }
    }
}

#[panic_handler]
fn rust_panic(_info: &PanicInfo) -> ! {
    loop {}
}
