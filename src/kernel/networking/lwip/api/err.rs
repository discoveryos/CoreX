//! Error Management module for lwIP in Rust

#[cfg(not(feature = "no_sys"))]
mod err {
    use libc::*; // for errno constants

    /// lwIP error type (negative numbers for errors, 0 for OK)
    pub type ErrT = i32;

    /// Table mapping lwIP errors to standard errno values
    const ERR_TO_ERRNO_TABLE: [i32; 17] = [
        0,           // ERR_OK          0
        ENOMEM,      // ERR_MEM        -1
        ENOBUFS,     // ERR_BUF        -2
        EWOULDBLOCK, // ERR_TIMEOUT    -3
        EHOSTUNREACH,// ERR_RTE        -4
        EINPROGRESS, // ERR_INPROGRESS -5
        EINVAL,      // ERR_VAL        -6
        EWOULDBLOCK, // ERR_WOULDBLOCK -7
        EADDRINUSE,  // ERR_USE        -8
        EALREADY,    // ERR_ALREADY    -9
        EISCONN,     // ERR_ISCONN     -10
        ENOTCONN,    // ERR_CONN       -11
        -1,          // ERR_IF         -12
        ECONNABORTED,// ERR_ABRT       -13
        ECONNRESET,  // ERR_RST        -14
        ENOTCONN,    // ERR_CLSD       -15
        EIO,         // ERR_ARG        -16
    ];

    /// Convert lwIP error to standard errno
    pub fn err_to_errno(err: ErrT) -> i32 {
        if err > 0 || (-err as usize) >= ERR_TO_ERRNO_TABLE.len() {
            return EIO;
        }
        ERR_TO_ERRNO_TABLE[-err as usize]
    }

    #[cfg(feature = "lwip_debug")]
    const ERR_STRERR: [&str; 17] = [
        "Ok.",                    // ERR_OK          0
        "Out of memory error.",   // ERR_MEM        -1
        "Buffer error.",          // ERR_BUF        -2
        "Timeout.",               // ERR_TIMEOUT    -3
        "Routing problem.",       // ERR_RTE        -4
        "Operation in progress.", // ERR_INPROGRESS -5
        "Illegal value.",         // ERR_VAL        -6
        "Operation would block.", // ERR_WOULDBLOCK -7
        "Address in use.",        // ERR_USE        -8
        "Already connecting.",    // ERR_ALREADY    -9
        "Already connected.",     // ERR_ISCONN     -10
        "Not connected.",         // ERR_CONN       -11
        "Low-level netif error.", // ERR_IF         -12
        "Connection aborted.",    // ERR_ABRT       -13
        "Connection reset.",      // ERR_RST        -14
        "Connection closed.",     // ERR_CLSD       -15
        "Illegal argument."       // ERR_ARG        -16
    ];

    /// Convert lwIP error to a string representation (debug)
    #[cfg(feature = "lwip_debug")]
    pub fn lwip_strerr(err: ErrT) -> &'static str {
        if err > 0 || (-err as usize) >= ERR_STRERR.len() {
            return "Unknown error.";
        }
        ERR_STRERR[-err as usize]
    }
}

// ----- Example usage -----
#[cfg(test)]
mod tests {
    use super::err::*;

    #[test]
    fn test_err_to_errno() {
        assert_eq!(err_to_errno(0), 0); // ERR_OK
        assert_eq!(err_to_errno(-1), libc::ENOMEM); // ERR_MEM
        assert_eq!(err_to_errno(5), libc::EIO); // positive => EIO
        assert_eq!(err_to_errno(-99), libc::EIO); // out of range => EIO
    }

    #[cfg(feature = "lwip_debug")]
    #[test]
    fn test_lwip_strerr() {
        assert_eq!(lwip_strerr(0), "Ok.");
        assert_eq!(lwip_strerr(-1), "Out of memory error.");
        assert_eq!(lwip_strerr(-99), "Unknown error.");
    }
}
