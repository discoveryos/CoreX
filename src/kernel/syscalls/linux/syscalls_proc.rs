use crate::acpi::*;
use crate::bootloader::*;
use crate::elf::*;
use crate::linked_list::*;
use crate::linux::*;
use crate::malloc::*;
use crate::syscalls::*;
use crate::system::*;
use crate::task::*;
use crate::util::*;
use core::ptr;

// ==========================
// Syscall: pipe
// ==========================
pub fn syscall_pipe(fds: &mut [i32; 2]) -> Result<usize, i32> {
    pipe_open(fds)
}

// ==========================
// Syscall: pipe2
// ==========================
pub fn syscall_pipe2(fds: &mut [i32; 2], flags: i32) -> Result<usize, i32> {
    if flags & !(O_CLOEXEC | O_NONBLOCK) != 0 {
        dbg_sys_stubf("todo flags");
        return Err(ENOSYS);
    }

    let out = pipe_open(fds)?;
    if flags != 0 {
        let fd0 = fs_user_get_node(current_task(), fds[0]).ok_or(EBADF)?;
        let fd1 = fs_user_get_node(current_task(), fds[1]).ok_or(EBADF)?;

        if flags & O_CLOEXEC != 0 {
            fd0.close_on_exec = true;
            fd1.close_on_exec = true;
        }
        if flags & O_NONBLOCK != 0 {
            fd0.flags |= O_NONBLOCK;
            fd1.flags |= O_NONBLOCK;
        }
    }
    Ok(out)
}

// ==========================
// Syscall: sched_yield
// ==========================
pub fn syscall_sched_yield() -> usize {
    hand_control();
    0
}

// ==========================
// Syscall: clone
// ==========================
pub fn syscall_clone(
    flags: u64,
    newsp: u64,
    parent_tid: Option<&mut i32>,
    child_tid: Option<&mut i32>,
    tls: u64,
) -> Result<usize, i32> {
    let supported_flags = CLONE_VFORK
        | CLONE_VM
        | CLONE_FILES
        | CLONE_SYSVSEM
        | CLONE_CHILD_CLEARTID
        | CLONE_PARENT_SETTID
        | CLONE_DETACHED
        | CLONE_THREAD
        | CLONE_SETTLS
        | CLONE_FS
        | CLONE_SIGHAND
        | 17; // SIGCHLD

    if flags & !supported_flags != 0 {
        dbg_sys_stubf(&format!("todo more flags {:x}", flags & !supported_flags));
        return Err(ENOSYS);
    }

    let mut flags = flags;
    if flags & CLONE_VFORK != 0 {
        flags |= CLONE_VM | CLONE_FILES;
    }

    let new_task = task_fork(current_task().syscall_regs, if newsp != 0 { newsp } else { current_task().syscall_rsp }, flags, false);
    let id = new_task.id;

    if flags & CLONE_SETTLS != 0 {
        new_task.fsbase = tls;
    }
    if flags & CLONE_CHILD_CLEARTID != 0 {
        if let Some(child_tid_ref) = child_tid {
            new_task.tidptr = Some(child_tid_ref);
        }
    }
    if flags & CLONE_PARENT_SETTID != 0 {
        if let Some(parent_tid_ref) = parent_tid {
            *parent_tid_ref = new_task.id as i32;
        }
    }

    task_create_finish(new_task);

    if flags & CLONE_VFORK != 0 {
        current_task().state = TaskState::WaitingVfork;
        hand_control();
    }

    Ok(id as usize)
}

// ==========================
// Syscall: fork
// ==========================
pub fn syscall_fork() -> usize {
    task_fork(current_task().syscall_regs, current_task().syscall_rsp, 0, true).id as usize
}

// ==========================
// Syscall: vfork
// ==========================
pub fn syscall_vfork() -> usize {
    let new_task = task_fork(current_task().syscall_regs, current_task().syscall_rsp, CLONE_VM, false);
    task_create_finish(new_task);
    current_task().state = TaskState::WaitingVfork;
    hand_control();
    new_task.id as usize
}

// ==========================
// Helper: copy pointer style for execve
// ==========================
pub struct CopyPtrStyle {
    pub count: usize,
    pub ptr_place: Vec<*mut u8>,
    pub val_place: Vec<u8>,
}

pub fn copy_ptr_style(ptrs: &[&str]) -> CopyPtrStyle {
    let count = ptrs.len();
    let total_len: usize = ptrs.iter().map(|s| s.len() + 1).sum();

    let mut val_place = vec![0u8; total_len];
    let mut ptr_place = Vec::with_capacity(count);

    let mut offset = 0;
    for s in ptrs {
        let bytes = s.as_bytes();
        ptr_place.push(val_place[offset..].as_mut_ptr());
        val_place[offset..offset + bytes.len()].copy_from_slice(bytes);
        val_place[offset + bytes.len()] = 0;
        offset += bytes.len() + 1;
    }

    CopyPtrStyle {
        count,
        ptr_place,
        val_place,
    }
}

// ==========================
// Syscall: execve
// ==========================
pub fn syscall_execve(filename: &str, argv: &[&str], envp: &[&str]) -> Result<usize, i32> {
    assert!(!argv.is_empty());

    let filename_sanitized = fs_sanitize(current_task().info_fs.cwd, filename);
    let mut buff = vec![0u8; 256];

    // pre-scan the file
    let pre_scan = fs_kernel_open(&filename_sanitized, O_RDONLY, 0).ok_or(ENOENT)?;
    let max = fs_read(&pre_scan, &mut buff[..255])?;
    fs_kernel_close(pre_scan);

    if max > 2 && buff[0] == b'#' && buff[1] == b'!' {
        // Shebang support, omitted for brevity (can recurse)
    } else if max > 4 && &buff[EI_MAG0..=EI_MAG3] == ELFMAG {
        // ELF executable
    } else {
        return Err(ENOEXEC);
    }

    let arguments = copy_ptr_style(argv);
    let environment = copy_ptr_style(envp);

    let ret = elf_execute(
        &filename_sanitized,
        arguments.count,
        &arguments.ptr_place,
        environment.count,
        &environment.ptr_place,
        0,
    )
    .ok_or(ENOENT)?;

    // Clone task metadata
    ret.id = current_task().id;
    ret.tgid = current_task().tgid;
    ret.parent = current_task().parent;
    ret.pgid = current_task().pgid;
    ret.sid = current_task().sid;
    ret.ctrl_pty = current_task().ctrl_pty;

    task_create_finish(ret);
    task_kill(current_task().id, 0);
    Ok(0)
}

// ==========================
// Syscall: exit_task
// ==========================
pub fn syscall_exit_task(return_code: i32) {
    task_kill(current_task().id, return_code);
}

// ==========================
// Syscall: wait4
// ==========================
pub fn syscall_wait4(pid: i32, wstatus: Option<&mut i32>, _options: i32, _ru: Option<&mut RUsage>) -> Result<usize, i32> {
    asm!("sti"); // enable interrupts

    if current_task().children_terminated_amnt == 0 {
        let mut amnt = 0;
        let browse = first_task();
        while let Some(task) = browse {
            if task.state == TaskState::Ready && task.parent == Some(current_task()) {
                amnt += 1;
            }
        }
        if amnt == 0 {
            return Err(ECHILD);
        }
    }

    // TODO: implement full wait4 logic
    Ok(0) // simplified
}

// ==========================
// Syscall: reboot
// ==========================
pub fn syscall_reboot(magic1: u32, magic2: u32, cmd: u32, _arg: *mut u8) -> Result<usize, i32> {
    if magic1 != LINUX_REBOOT_MAGIC1 || magic2 != LINUX_REBOOT_MAGIC2 {
        return Err(EINVAL);
    }
    match cmd {
        LINUX_REBOOT_CMD_POWER_OFF => Ok(acpi_poweroff()),
        LINUX_REBOOT_CMD_RESTART => Ok(acpi_reboot()),
        _ => Err(EINVAL),
    }
}

// ==========================
// Syscall: futex
// ==========================
pub fn syscall_futex(addr: &mut u32, op: i32, value: u32, utime: Option<&mut Timespec>, addr2: Option<&mut u32>, value3: u32) -> usize {
    if current_task().extras & EXTRAS_DISABLE_FUTEX != 0 {
        return 0;
    }
    futex_syscall(addr, op, value, utime, addr2, value3)
}

// ==========================
// Syscall: exit_group
// ==========================
pub fn syscall_exit_group(return_code: i32) {
    let browse = first_task();
    while let Some(task) = browse {
        if task.tgid == current_task().tgid && task.id != current_task().id {
            task.sig_pending_list.set(SIGKILL);
        }
    }
    syscall_exit_task(return_code);
}

// ==========================
// Syscall: eventfd2
// ==========================
pub fn syscall_eventfd2(init_value: u64, flags: i32) -> Result<usize, i32> {
    Ok(event_fd_open(init_value, flags))
}

// ==========================
// Register process syscalls
// ==========================
pub fn syscalls_reg_proc() {
    register_syscall(SYSCALL_SCHED_YIELD, syscall_sched_yield as usize);
    register_syscall(SYSCALL_PIPE, syscall_pipe as usize);
    register_syscall(SYSCALL_PIPE2, syscall_pipe2 as usize);
    register_syscall(SYSCALL_EXIT_TASK, syscall_exit_task as usize);
    register_syscall(SYSCALL_CLONE, syscall_clone as usize);
    register_syscall(SYSCALL_FORK, syscall_fork as usize);
    register_syscall(SYSCALL_VFORK, syscall_vfork as usize);
    register_syscall(SYSCALL_WAIT4, syscall_wait4 as usize);
    register_syscall(SYSCALL_EXECVE, syscall_execve as usize);
    register_syscall(SYSCALL_EXIT_GROUP, syscall_exit_group as usize);
    register_syscall(SYSCALL_REBOOT, syscall_reboot as usize);
    register_syscall(SYSCALL_EVENTFD2, syscall_eventfd2 as usize);
    register_syscall(SYSCALL_FUTEX, syscall_futex as usize);
}
