use std::ptr;
use std::sync::{Arc, Mutex};
use std::os::raw::{c_int, c_void};
use std::io::{Error, Result};
use libc::{c_uint, size_t, sockaddr};

extern "C" {
    fn lwip_send(fd: c_int, buf: *const u8, len: size_t, flags: c_int) -> c_int;
    fn lwip_recv(fd: c_int, buf: *mut u8, len: size_t, flags: c_int) -> c_int;
    fn lwip_close(fd: c_int) -> c_int;
    fn lwip_bind(fd: c_int, addr: *const sockaddr, len: size_t) -> c_int;
    fn lwip_connect(fd: c_int, addr: *const sockaddr, len: c_uint) -> c_int;
    fn lwip_listen(fd: c_int, backlog: c_int) -> c_int;
    fn lwip_recvfrom(fd: c_int, buf: *mut u8, len: size_t, flags: c_int, addr: *mut sockaddr, addrlen: *mut c_uint) -> c_int;
    fn lwip_sendto(fd: c_int, buf: *const u8, len: size_t, flags: c_int, addr: *const sockaddr, addrlen: c_uint) -> c_int;
}

#[derive(Clone)]
pub struct UserSocket {
    lwip_fd: c_int,
    socket_instances: Arc<Mutex<usize>>,
}

impl UserSocket {
    pub fn send(&self, buf: &[u8], flags: c_int, nonblock: bool) -> Result<usize> {
        loop {
            // TODO: replace this with proper epoll / poll integration
            let res = unsafe { lwip_send(self.lwip_fd, buf.as_ptr(), buf.len(), flags) };
            if res >= 0 {
                return Ok(res as usize);
            } else {
                let err = Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock && nonblock {
                    return Err(Error::from_raw_os_error(libc::EAGAIN));
                } else if err.kind() == std::io::ErrorKind::Interrupted {
                    return Err(Error::from_raw_os_error(libc::EINTR));
                }
            }
        }
    }

    pub fn recv(&self, buf: &mut [u8], flags: c_int, nonblock: bool) -> Result<usize> {
        loop {
            let res = unsafe { lwip_recv(self.lwip_fd, buf.as_mut_ptr(), buf.len(), flags) };
            if res >= 0 {
                return Ok(res as usize);
            } else {
                let err = Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock && nonblock {
                    return Err(Error::from_raw_os_error(libc::EAGAIN));
                } else if err.kind() == std::io::ErrorKind::Interrupted {
                    return Err(Error::from_raw_os_error(libc::EINTR));
                }
            }
        }
    }

    pub fn close(&self) -> Result<()> {
        let mut instances = self.socket_instances.lock().unwrap();
        *instances -= 1;
        if *instances == 0 {
            let res = unsafe { lwip_close(self.lwip_fd) };
            if res < 0 {
                return Err(Error::last_os_error());
            }
        }
        Ok(())
    }

    pub fn duplicate(&self) -> UserSocket {
        let mut instances = self.socket_instances.lock().unwrap();
        *instances += 1;
        self.clone()
    }

    pub fn bind(&self, addr: *const sockaddr, len: size_t) -> Result<()> {
        let res = unsafe { lwip_bind(self.lwip_fd, addr, len) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn connect(&self, addr: *const sockaddr, len: c_uint, nonblock: bool) -> Result<()> {
        // TODO: handle non-blocking fcntl if needed
        let res = unsafe { lwip_connect(self.lwip_fd, addr, len) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn listen(&self, backlog: c_int) -> Result<()> {
        let res = unsafe { lwip_listen(self.lwip_fd, backlog) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn sendto(&self, buf: &[u8], flags: c_int, addr: *const sockaddr, addrlen: c_uint) -> Result<usize> {
        let res = unsafe { lwip_sendto(self.lwip_fd, buf.as_ptr(), buf.len(), flags, addr, addrlen) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(res as usize)
    }

    pub fn recvfrom(&self, buf: &mut [u8], flags: c_int, addr: *mut sockaddr, addrlen: &mut c_uint) -> Result<usize> {
        let res = unsafe { lwip_recvfrom(self.lwip_fd, buf.as_mut_ptr(), buf.len(), flags, addr, addrlen) };
        if res < 0 {
            return Err(Error::last_os_error());
        }
        Ok(res as usize)
    }
}
