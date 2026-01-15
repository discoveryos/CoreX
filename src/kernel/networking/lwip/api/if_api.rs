//! Interface Identification APIs (RFC 3493) for Rust/lwIP

#[cfg(feature = "lwip_socket")]
pub mod if_api {
    use crate::errno;
    use crate::netifapi; // your Rust wrapper around lwIP netif API

    /// Maps an interface index to its name.
    ///
    /// Returns `Some(String)` if the index is valid; otherwise `None` and sets errno.
    pub fn if_indextoname(ifindex: u32) -> Option<String> {
        #[cfg(feature = "lwip_netif_api")]
        {
            if ifindex <= 0xff {
                match netifapi::netif_index_to_name(ifindex as u8) {
                    Ok(name) if !name.is_empty() => Some(name),
                    _ => {}
                }
            }
        }

        // If not available or invalid index
        errno::set_errno(errno::ENXIO);
        None
    }

    /// Maps an interface name to its index.
    ///
    /// Returns `Some(u32)` if the interface exists, otherwise `None`.
    pub fn if_nametoindex(ifname: &str) -> Option<u32> {
        #[cfg(feature = "lwip_netif_api")]
        {
            match netifapi::netif_name_to_index(ifname) {
                Ok(idx) => Some(idx as u32),
                Err(_) => {}
            }
        }
        None
    }
}

// ------------------ Example usage ------------------
// Assuming you have a `netifapi` Rust module with safe wrappers

#[cfg(test)]
mod tests {
    use super::if_api::*;
    use crate::errno;

    #[test]
    fn test_if_indextoname_invalid() {
        assert_eq!(if_indextoname(999), None);
        assert_eq!(errno::get_errno(), errno::ENXIO);
    }

    #[test]
    fn test_if_nametoindex_invalid() {
        assert_eq!(if_nametoindex("nonexistent0"), None);
    }
}
