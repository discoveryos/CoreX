#![no_std]

use core::arch::asm;
use core::ptr;

// ============================
// uACPI FFI
// ============================

type UacpiStatus = i32;
type SizeT = usize;

const UACPI_SLEEP_STATE_S5: u32 = 5;
const UACPI_INTERRUPT_MODEL_IOAPIC: u32 = 1;

const EIO: usize = 5;

// ============================
// External uACPI symbols
// ============================

extern "C" {
    fn uacpi_initialize(flags: u32) -> UacpiStatus;
    fn uacpi_namespace_load() -> UacpiStatus;
    fn uacpi_namespace_initialize() -> UacpiStatus;

    fn uacpi_prepare_for_sleep_state(state: u32) -> UacpiStatus;
    fn uacpi_enter_sleep_state(state: u32) -> UacpiStatus;
    fn uacpi_reboot() -> UacpiStatus;

    fn uacpi_set_interrupt_model(model: u32);

    fn uacpi_status_to_string(status: UacpiStatus) -> *const u8;
    fn uacpi_table_fadt(out: *mut *const AcpiFadt) -> UacpiStatus;
}

// ============================
// ACPI tables (partial)
// ============================

#[repr(C)]
struct AcpiFadt {
    _reserved: [u8; 108],
    century: u8,
}

// ============================
// Kernel-provided symbols
// ============================

extern "C" {
    fn debugf(fmt: *const u8, ...) ;
    fn kernel_panic() -> !;
}

// ============================
// Globals
// ============================

static mut CENTURY_REGISTER: u8 = 0;

// ============================
// Helpers
// ============================

#[inline(always)]
fn unlikely_error(status: UacpiStatus) -> bool {
    status != 0
}

#[inline(always)]
fn err(code: usize) -> usize {
    usize::MAX - code + 1
}

// ============================
// ACPI Initialization
// ============================

pub fn initiate_acpi() {
    unsafe {
        let mut ret = uacpi_initialize(0);
        if unlikely_error(ret) {
            debugf(
                b"uacpi_initialize error: %s\n\0".as_ptr(),
                uacpi_status_to_string(ret),
            );
            kernel_panic();
        }

        ret = uacpi_namespace_load();
        if unlikely_error(ret) {
            debugf(
                b"uacpi_namespace_load error: %s\n\0".as_ptr(),
                uacpi_status_to_string(ret),
            );
            kernel_panic();
        }

        uacpi_set_interrupt_model(UACPI_INTERRUPT_MODEL_IOAPIC);

        ret = uacpi_namespace_initialize();
        if unlikely_error(ret) {
            debugf(
                b"uacpi_namespace_initialize error: %s\n\0".as_ptr(),
                uacpi_status_to_string(ret),
            );
            kernel_panic();
        }

        let mut fadt: *const AcpiFadt = ptr::null();
        if !unlikely_error(uacpi_table_fadt(&mut fadt)) && !fadt.is_null() {
            CENTURY_REGISTER = (*fadt).century;
            if CENTURY_REGISTER != 0 {
                debugf(
                    b"[acpi::info] Found century register for RTC: register=0x%lx\n\0"
                        .as_ptr(),
                    CENTURY_REGISTER as usize,
                );
            }
        }
    }
}

// ============================
// Poweroff
// ============================

pub fn acpi_poweroff() -> SizeT {
    unsafe {
        let ret = uacpi_prepare_for_sleep_state(UACPI_SLEEP_STATE_S5);
        if unlikely_error(ret) {
            debugf(
                b"[acpi] Couldn't prepare for poweroff: %s\n\0".as_ptr(),
                uacpi_status_to_string(ret),
            );
            return err(EIO);
        }

        asm!("cli", options(nomem, nostack, preserves_flags));

        let ret_poweroff = uacpi_enter_sleep_state(UACPI_SLEEP_STATE_S5);
        if unlikely_error(ret_poweroff) {
            asm!("sti", options(nomem, nostack, preserves_flags));
            debugf(
                b"[acpi] Couldn't power off the system: %s\n\0".as_ptr(),
                uacpi_status_to_string(ret_poweroff),
            );
            return err(EIO);
        }

        debugf(b"[acpi] Shouldn't be reached after power off!\n\0".as_ptr());
        kernel_panic();
    }
}

// ============================
// Reboot
// ============================

pub fn acpi_reboot() -> SizeT {
    unsafe {
        let _ = uacpi_prepare_for_sleep_state(UACPI_SLEEP_STATE_S5);

        let ret = uacpi_reboot();
        if unlikely_error(ret) {
            debugf(
                b"[acpi] Couldn't restart the system: %s\n\0".as_ptr(),
                uacpi_status_to_string(ret),
            );
            return err(EIO);
        }

        debugf(b"[acpi] Shouldn't be reached after reboot!\n\0".as_ptr());
        kernel_panic();
    }
}
