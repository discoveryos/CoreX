use crate::vfs::{fs_sanitize, fs_resolve_symlink, fs_strip_mountpoint, fs_determine_mountpoint, MountPoint, OpenFile};
use crate::task::Task;

/// File stat structure stub
#[derive(Default)]
pub struct Stat {
    // Fill in fields as needed (size, mode, timestamps, etc.)
    pub size: usize,
    pub mode: u32,
    pub is_dir: bool,
}

/// Generic fsStat for an already-open file
pub fn fs_stat(fd: &OpenFile, target: &mut Stat) -> bool {
    if let Some(stat_handler) = fd.handlers.stat {
        stat_handler(fd, target) == 0
    } else {
        false
    }
}

/// fsStat by filename (resolves symlinks)
pub fn fs_stat_by_filename(task: &Task, filename: &str, target: &mut Stat) -> bool {
    let safe_filename = {
        let info_fs = &task.info_fs;
        let _lock = info_fs.lock_fs.lock().unwrap();
        fs_sanitize(&info_fs.cwd, filename)
    };

    let mnt = fs_determine_mountpoint(&safe_filename);
    if mnt.is_none() {
        return false;
    }
    let mnt = mnt.unwrap();
    let stripped_filename = fs_strip_mountpoint(&safe_filename, mnt);

    if mnt.stat.is_none() {
        return false;
    }

    let mut symlink: Option<String> = None;
    let ret = mnt.stat.unwrap()(mnt, &stripped_filename, target, &mut symlink);

    if !ret {
        if let Some(link) = symlink {
            let symlink_resolved = fs_resolve_symlink(mnt, &link);
            return fs_stat_by_filename(task, &symlink_resolved, target);
        }
    }

    ret
}

/// fsLstat by filename (like stat but does not follow final symlink)
pub fn fs_lstat_by_filename(task: &Task, filename: &str, target: &mut Stat) -> bool {
    let safe_filename = {
        let info_fs = &task.info_fs;
        let _lock = info_fs.lock_fs.lock().unwrap();
        fs_sanitize(&info_fs.cwd, filename)
    };

    let mnt = fs_determine_mountpoint(&safe_filename);
    if mnt.is_none() {
        return false;
    }
    let mnt = mnt.unwrap();
    let stripped_filename = fs_strip_mountpoint(&safe_filename, mnt);

    if mnt.lstat.is_none() {
        return false;
    }

    let mut symlink: Option<String> = None;
    let ret = mnt.lstat.unwrap()(mnt, &stripped_filename, target, &mut symlink);

    if !ret {
        if let Some(link) = symlink {
            let symlink_resolved = fs_resolve_symlink(mnt, &link);
            return fs_lstat_by_filename(task, &symlink_resolved, target);
        }
    }

    ret
}
