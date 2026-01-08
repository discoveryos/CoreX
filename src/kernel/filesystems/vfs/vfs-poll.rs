/// VFS-sided implementation of generic `poll()` functions
/// (C) 2025 kevin dan mathew

use crate::vfs::OpenFile;

/// Inform the VFS that a file descriptor is ready for certain events.
/// Currently a stub (no-op), can be expanded for epoll-like behavior.
pub fn fs_inform_ready(_fd: &mut OpenFile, _epoll_events: u32) {
    // In the original C code, this is a stub:
    // spinlockAcquire(&fd->LOCK_POLL);
    // // do nothing
    // spinlockRelease(&fd->LOCK_POLL);

    // In Rust, we simply do nothing for now.
}
