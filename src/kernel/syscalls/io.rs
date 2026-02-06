use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

// Simulated framebuffer and keyboard task structures
struct Fb {
    width: usize,
    height: usize,
}

struct Termios {
    lflag: u32,
}

struct Task {
    id: usize,
    term: Termios,
    tmp_recv: usize,
    state_waiting_input: bool,
}

static FB: Fb = Fb {
    width: 800,
    height: 600,
};

lazy_static::lazy_static! {
    static ref CURRENT_TASK: Arc<Mutex<Task>> = Arc::new(Mutex::new(Task {
        id: 1,
        term: Termios { lflag: 0 },
        tmp_recv: 0,
        state_waiting_input: false,
    }));
}

struct Winsize {
    ws_row: usize,
    ws_col: usize,
    ws_xpixel: usize,
    ws_ypixel: usize,
}

struct OpenFile {
    // In kernel this holds more info; here minimal
}

struct Stat {
    st_dev: u64,
    st_ino: u64,
    st_mode: u32,
    st_nlink: u32,
    st_uid: u32,
    st_gid: u32,
    st_rdev: u64,
    st_blksize: u64,
    st_size: u64,
    st_blocks: u64,
    st_atime: u64,
    st_mtime: u64,
    st_ctime: u64,
}

// ------------------------- Handlers -------------------------

fn read_handler(_fd: &OpenFile, buffer: &mut [u8]) -> io::Result<usize> {
    // simulate blocking keyboard read
    let mut task = CURRENT_TASK.lock().unwrap();

    task.state_waiting_input = true;

    // simulate reading input
    let limit = buffer.len();
    let mut kernel_buf = vec![0u8; limit];
    // kbTaskRead would fill this buffer
    for i in 0..limit {
        kernel_buf[i] = b'a'; // mock input
    }

    // simulate end of input
    task.tmp_recv = limit;
    task.state_waiting_input = false;

    // copy to caller buffer
    buffer[..task.tmp_recv].copy_from_slice(&kernel_buf[..task.tmp_recv]);

    // add newline if ICANON is set (mock behavior)
    if task.term.lflag & 0x02 != 0 && task.tmp_recv < limit {
        buffer[task.tmp_recv] = b'\n';
        task.tmp_recv += 1;
    }

    Ok(task.tmp_recv)
}

fn write_handler(_fd: &OpenFile, buffer: &[u8]) -> io::Result<usize> {
    for &b in buffer {
        print!("{}", b as char);
    }
    Ok(buffer.len())
}

fn ioctl_handler(_fd: &OpenFile, request: u64, arg: &mut [u8]) -> i32 {
    match request {
        0x5413 => {
            // TIOCGWINSZ
            let win: &mut Winsize =
                unsafe { &mut *(arg.as_mut_ptr() as *mut Winsize) };
            win.ws_row = FB.height / 16;
            win.ws_col = FB.width / 8;
            win.ws_xpixel = FB.width;
            win.ws_ypixel = FB.height;
            0
        }
        0x540a => 0, // dummy
        0x5401 => {
            // TCGETS
            let term: &mut Termios =
                unsafe { &mut *(arg.as_mut_ptr() as *mut Termios) };
            let task = CURRENT_TASK.lock().unwrap();
            *term = task.term;
            0
        }
        0x5402 | 0x5403 | 0x5404 => {
            // TCSETS, TCSETSW, TCSETSF
            let term: &Termios =
                unsafe { &*(arg.as_ptr() as *const Termios) };
            let mut task = CURRENT_TASK.lock().unwrap();
            task.term = *term;
            0
        }
        0x540f => {
            // TIOCGPGRP
            let pid: &mut u32 = unsafe { &mut *(arg.as_mut_ptr() as *mut u32) };
            let task = CURRENT_TASK.lock().unwrap();
            *pid = task.id as u32;
            0
        }
        _ => -1,
    }
}

fn mmap_handler(
    _addr: usize,
    _length: usize,
    _prot: i32,
    _flags: i32,
    _fd: &OpenFile,
    _pgoffset: usize,
) -> isize {
    eprintln!("[io::mmap] FATAL! Tried to mmap on stdio!");
    -1
}

fn stat_handler(_fd: &OpenFile, target: &mut Stat) -> i32 {
    target.st_dev = 420;
    target.st_ino = rand::random();
    target.st_mode = 0o20660; // S_IFCHR | S_IRUSR | S_IWUSR
    target.st_nlink = 1;
    target.st_uid = 0;
    target.st_gid = 0;
    target.st_rdev = 34830;
    target.st_blksize = 0x1000;
    target.st_size = 0;
    target.st_blocks = (target.st_size + 511) / 512;
    target.st_atime = 69;
    target.st_mtime = 69;
    target.st_ctime = 69;
    0
}

static mut IO_SWITCH: bool = false;

fn internal_poll_handler(_fd: &OpenFile, events: i32) -> i32 {
    let mut revents = 0;
    unsafe {
        if events & 0x001 != 0 && IO_SWITCH {
            revents |= 0x001; // EPOLLIN
        }
        if events & 0x004 != 0 {
            revents |= 0x004; // EPOLLOUT
        }
        IO_SWITCH = !IO_SWITCH;
    }
    revents
}

fn report_key_handler(_fd: &OpenFile) -> usize {
    69
}

// ---------------------- VfsHandlers Struct ----------------------

struct VfsHandlers {
    read: fn(&OpenFile, &mut [u8]) -> io::Result<usize>,
    write: fn(&OpenFile, &[u8]) -> io::Result<usize>,
    ioctl: fn(&OpenFile, u64, &mut [u8]) -> i32,
    mmap: fn(usize, usize, i32, i32, &OpenFile, usize) -> isize,
    stat: fn(&OpenFile, &mut Stat) -> i32,
    internal_poll: fn(&OpenFile, i32) -> i32,
    report_key: fn(&OpenFile) -> usize,
}

static STDIO: VfsHandlers = VfsHandlers {
    read: read_handler,
    write: write_handler,
    ioctl: ioctl_handler,
    mmap: mmap_handler,
    stat: stat_handler,
    internal_poll: internal_poll_handler,
    report_key: report_key_handler,
};

// ---------------------- Example Usage ----------------------

fn main() {
    let fd = OpenFile {};
    let mut buf = [0u8; 32];

    let n = (STDIO.read)(&fd, &mut buf).unwrap();
    println!("Read {} bytes: {:?}", n, &buf[..n]);

    (STDIO.write)(&fd, b"Hello from Rust stdio!\n").unwrap();

    let mut stat = Stat {
        st_dev: 0,
        st_ino: 0,
        st_mode: 0,
        st_nlink: 0,
        st_uid: 0,
        st_gid: 0,
        st_rdev: 0,
        st_blksize: 0,
        st_size: 0,
        st_blocks: 0,
        st_atime: 0,
        st_mtime: 0,
        st_ctime: 0,
    };
    (STDIO.stat)(&fd, &mut stat);
    println!("Stat: st_dev={}, st_mode={:#o}", stat.st_dev, stat.st_mode);
}
