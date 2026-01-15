//! Name resolution API (gethostbyname, getaddrinfo) in Rust

#[cfg(all(feature = "lwip_dns", feature = "lwip_socket"))]
pub mod netdb {
    use crate::ip_addr::IpAddr;
    use crate::err::{ErrT, ERR_OK, HOST_NOT_FOUND, EAI_FAIL, EAI_MEMORY, EAI_NONAME};
    use crate::api::netconn_gethostbyname;
    use std::ffi::CString;

    /// Equivalent to C `struct hostent`
    pub struct HostEnt<'a> {
        pub h_name: &'a str,
        pub h_aliases: Option<&'a [&'a str]>,
        pub h_addrtype: i32,
        pub h_length: usize,
        pub h_addr_list: &'a [&'a IpAddr],
    }

    /// Resolve hostname to a single IP address (IPv4)
    pub fn gethostbyname(name: &str) -> Result<HostEnt, ErrT> {
        let addr = netconn_gethostbyname(name)?;
        // in lwIP, only one address returned
        let addr_list = [&addr];
        Ok(HostEnt {
            h_name: name,
            h_aliases: None,
            h_addrtype: libc::AF_INET,
            h_length: std::mem::size_of::<IpAddr>(),
            h_addr_list: &addr_list,
        })
    }

    /// Thread-safe variant: fills pre-allocated `HostEnt` struct
    pub fn gethostbyname_r<'a>(
        name: &str,
        ret: &'a mut HostEnt<'a>,
        buf: &'a mut [u8],
    ) -> Result<&'a HostEnt<'a>, ErrT> {
        if buf.len() < name.len() {
            return Err(HOST_NOT_FOUND);
        }
        let addr = netconn_gethostbyname(name)?;

        // copy hostname into buffer
        let hostname_bytes = name.as_bytes();
        buf[..hostname_bytes.len()].copy_from_slice(hostname_bytes);
        let hostname = std::str::from_utf8(&buf[..hostname_bytes.len()]).unwrap();

        ret.h_name = hostname;
        ret.h_aliases = None;
        ret.h_addrtype = libc::AF_INET;
        ret.h_length = std::mem::size_of::<IpAddr>();
        ret.h_addr_list = &[&addr];

        Ok(ret)
    }

    /// Equivalent to C `struct addrinfo`
    pub struct AddrInfo<'a> {
        pub ai_family: i32,
        pub ai_socktype: i32,
        pub ai_protocol: i32,
        pub ai_addrlen: usize,
        pub ai_addr: &'a IpAddr,
        pub ai_canonname: Option<&'a str>,
        pub ai_next: Option<Box<AddrInfo<'a>>>,
    }

    /// Simplified getaddrinfo: only first IP and numeric ports supported
    pub fn getaddrinfo<'a>(
        nodename: Option<&str>,
        servname: Option<&str>,
        hints: Option<&AddrInfo>,
    ) -> Result<AddrInfo<'a>, ErrT> {
        let port = if let Some(s) = servname {
            s.parse::<u16>().map_err(|_| EAI_FAIL)?
        } else {
            0
        };

        let ai_family = hints.map_or(libc::AF_UNSPEC, |h| h.ai_family);

        let addr: IpAddr = if let Some(host) = nodename {
            netconn_gethostbyname(host)?
        } else {
            // use loopback or any address
            IpAddr::loopback(ai_family == libc::AF_INET6)
        };

        Ok(AddrInfo {
            ai_family,
            ai_socktype: hints.map_or(0, |h| h.ai_socktype),
            ai_protocol: hints.map_or(0, |h| h.ai_protocol),
            ai_addrlen: std::mem::size_of::<IpAddr>(),
            ai_addr: &addr,
            ai_canonname: nodename,
            ai_next: None,
        })
    }

    /// Free a linked list of AddrInfo (Rust drops automatically)
    pub fn freeaddrinfo(_ai: AddrInfo) {
        // Memory automatically freed in Rust
    }
}
