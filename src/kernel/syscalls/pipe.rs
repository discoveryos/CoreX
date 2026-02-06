use std::sync::{Arc, Mutex, Condvar};
use std::collections::VecDeque;
use std::io::{self, Read, Write};

const PIPE_BUFF: usize = 65536;

#[derive(Debug)]
struct PipeInfo {
    buf: VecDeque<u8>,
    read_fds: usize,
    write_fds: usize,
    cond_read: Condvar,
    cond_write: Condvar,
}

#[derive(Clone)]
struct PipeEnd {
    write: bool,
    pipe: Arc<Mutex<PipeInfo>>,
}

impl PipeInfo {
    fn new() -> Self {
        Self {
            buf: VecDeque::with_capacity(PIPE_BUFF),
            read_fds: 1,
            write_fds: 1,
            cond_read: Condvar::new(),
            cond_write: Condvar::new(),
        }
    }
}

impl PipeEnd {
    fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut pipe = self.pipe.lock().unwrap();

        while pipe.buf.is_empty() {
            if pipe.write_fds == 0 {
                return Ok(0); // EOF
            }
            pipe = self.cond_read.wait(pipe).unwrap();
        }

        let to_copy = buf.len().min(pipe.buf.len());
        for i in 0..to_copy {
            buf[i] = pipe.buf.pop_front().unwrap();
        }

        self.cond_write.notify_all();
        Ok(to_copy)
    }

    fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let mut pipe = self.pipe.lock().unwrap();
        let mut written = 0;

        while written < buf.len() {
            while pipe.buf.len() >= PIPE_BUFF {
                if pipe.read_fds == 0 {
                    return Err(io::Error::from(io::ErrorKind::BrokenPipe));
                }
                pipe = self.cond_write.wait(pipe).unwrap();
            }

            let space = PIPE_BUFF - pipe.buf.len();
            let to_write = (buf.len() - written).min(space);

            pipe.buf.extend(&buf[written..written + to_write]);
            written += to_write;

            self.cond_read.notify_all();
        }

        Ok(written)
    }

    fn close(&self) {
        let mut pipe = self.pipe.lock().unwrap();
        if self.write {
            pipe.write_fds = pipe.write_fds.saturating_sub(1);
            self.cond_read.notify_all();
        } else {
            pipe.read_fds = pipe.read_fds.saturating_sub(1);
            self.cond_write.notify_all();
        }
    }
}

fn pipe() -> (PipeEnd, PipeEnd) {
    let info = Arc::new(Mutex::new(PipeInfo::new()));
    let read_end = PipeEnd { write: false, pipe: info.clone() };
    let write_end = PipeEnd { write: true, pipe: info.clone() };
    (read_end, write_end)
}

// Example usage
fn main() {
    let (reader, writer) = pipe();

    std::thread::spawn(move || {
        writer.write(b"Hello from Rust pipe!").unwrap();
        writer.close();
    });

    let mut buffer = vec![0u8; 1024];
    let n = reader.read(&mut buffer).unwrap();
    println!("Read {} bytes: {:?}", n, &buffer[..n]);
    reader.close();
}
