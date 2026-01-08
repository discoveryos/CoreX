use alloc::{boxed::Box, string::String, vec::Vec};
use core::cell::RefCell;
use spin::Mutex;
use hashbrown::HashMap;

/// --- Filesystem Types ---
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSystem {
    Fat32,
    Ext2,
    Dev,
    Sys,
    Proc,
}

/// --- Connector Types ---
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connector {
    Ahci,
    Dev,
    Sys,
    Proc,
}

/// --- Master Boot Record Partition ---
#[derive(Clone)]
pub struct MbrPartition {
    pub lba_first_sector: u32,
    pub partition_type: u8,
}

/// --- MountPoint Structure ---
pub struct MountPoint {
    pub prefix: String,          // Must end with '/'
    pub disk: Option<u32>,
    pub partition: Option<u8>,
    pub connector: Connector,
    pub filesystem: FileSystem,
    pub mbr: Option<MbrPartition>,
}

/// --- Global mount points list (thread-safe) ---
pub struct MountManager {
    mounts: Mutex<Vec<Box<MountPoint>>>,
}

impl MountManager {
    pub fn new() -> Self {
        MountManager {
            mounts: Mutex::new(Vec::new()),
        }
    }

    /// Mount a new filesystem
    pub fn mount(
        &self,
        prefix: &str,
        connector: Connector,
        disk: Option<u32>,
        partition: Option<u8>,
    ) -> Option<&Box<MountPoint>> {
        let mut mounts = self.mounts.lock();

        let mut mount = Box::new(MountPoint {
            prefix: prefix.to_string(),
            disk,
            partition,
            connector,
            filesystem: match connector {
                Connector::Ahci => FileSystem::Fat32, // placeholder
                Connector::Dev => FileSystem::Dev,
                Connector::Sys => FileSystem::Sys,
                Connector::Proc => FileSystem::Proc,
            },
            mbr: None,
        });

        // Here we would check the disk type and call mount functions
        // For example:
        if connector == Connector::Ahci {
            if let Some(d) = disk {
                // Simulate reading MBR
                let mbr = MbrPartition {
                    lba_first_sector: 2048,
                    partition_type: 0x83,
                };
                mount.mbr = Some(mbr);
                // Detect FS type
                if mbr.partition_type == 0x83 {
                    mount.filesystem = FileSystem::Ext2;
                } else {
                    mount.filesystem = FileSystem::Fat32;
                }
            } else {
                return None;
            }
        }

        mounts.push(mount);
        mounts.last()
    }

    /// Unmount a filesystem
    pub fn unmount(&self, mount: &MountPoint) -> bool {
        let mut mounts = self.mounts.lock();
        if let Some(pos) = mounts.iter().position(|m| m.prefix == mount.prefix) {
            mounts.remove(pos);
            true
        } else {
            false
        }
    }

    /// Determine the mount point that best matches a filename
    pub fn determine_mount_point(&self, filename: &str) -> Option<&MountPoint> {
        let mounts = self.mounts.lock();
        let mut best: Option<&MountPoint> = None;
        let mut largest_len = 0;

        for mount in mounts.iter() {
            let len = mount.prefix.len().saturating_sub(1); // skip trailing '/'
            if filename.starts_with(&mount.prefix[..len])
                && (filename.len() == len || filename.as_bytes()[len] == b'/')
            {
                if len >= largest_len {
                    best = Some(mount);
                    largest_len = len;
                }
            }
        }

        best
    }

    /// Resolve a symlink
    pub fn resolve_symlink(&self, mount: &MountPoint, symlink: &str) -> Option<String> {
        let prefix_len = mount.prefix.len() - 1; // remove trailing '/'

        if symlink.starts_with('/') {
            Some(format!("{}{}", &mount.prefix[..prefix_len], symlink))
        } else if symlink.starts_with('!') {
            Some(symlink[1..].to_string())
        } else {
            None
        }
    }
}
