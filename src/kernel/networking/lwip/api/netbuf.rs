//! Network buffer management for Rust/lwIP (`netbuf`)

#[cfg(feature = "lwip_netconn")]
pub mod netbuf {
    use crate::err::{self, ErrT, ERR_OK, ERR_MEM, ERR_BUF, ERR_ARG};
    use crate::pbuf::{self, PBuf, PBufType, PBufAllocError};

    /// Network buffer descriptor
    pub struct NetBuf {
        pub p: Option<Box<PBuf>>, // allocated pbuf
        pub ptr: Option<*mut PBuf>, // current pointer into pbuf chain
        #[cfg(feature = "lwip_checksum_on_copy")]
        pub flags: u8,
        #[cfg(feature = "lwip_checksum_on_copy")]
        pub toport_chksum: u16,
    }

    impl NetBuf {
        /// Create a new empty netbuf (no pbuf allocated yet)
        pub fn new() -> Option<Self> {
            Some(Self {
                p: None,
                ptr: None,
                #[cfg(feature = "lwip_checksum_on_copy")]
                flags: 0,
                #[cfg(feature = "lwip_checksum_on_copy")]
                toport_chksum: 0,
            })
        }

        /// Free the netbuf and its packet buffer
        pub fn delete(self) {
            // Rust drops automatically, pbuf freed when Box<PBuf> dropped
        }

        /// Allocate a packet buffer of given size inside the netbuf
        pub fn alloc(&mut self, size: usize) -> Option<&mut [u8]> {
            // Free previous pbuf if exists
            self.p = None;

            match PBuf::alloc(PBufType::Transport, size) {
                Ok(mut pb) => {
                    self.ptr = Some(&mut *pb as *mut _);
                    let payload = pb.payload_mut();
                    self.p = Some(pb);
                    Some(payload)
                }
                Err(_) => None,
            }
        }

        /// Free the packet buffer in a netbuf
        pub fn free(&mut self) {
            self.p = None;
            self.ptr = None;
            #[cfg(feature = "lwip_checksum_on_copy")]
            {
                self.flags = 0;
                self.toport_chksum = 0;
            }
        }

        /// Reference existing data without copying
        pub fn reference(&mut self, data: &[u8]) -> Result<(), ErrT> {
            self.p = None;

            let mut pb = PBuf::alloc_ref(data.len(), data)?;
            self.ptr = Some(&mut *pb as *mut _);
            self.p = Some(pb);
            Ok(())
        }

        /// Chain another netbuf after this one
        pub fn chain(&mut self, mut tail: NetBuf) {
            if let (Some(ref mut head_p), Some(ref mut tail_p)) = (&mut self.p, &mut tail.p) {
                pbuf::cat(head_p, tail_p);
                self.ptr = Some(&mut **head_p as *mut _);
            }
            // tail is freed automatically when dropped
        }

        /// Get the data pointer and length from the current pbuf in the netbuf
        pub fn data(&self) -> Result<(&[u8], usize), ErrT> {
            let ptr = self.ptr.ok_or(ERR_BUF)?;
            unsafe {
                let pb = &*ptr;
                Ok((pb.payload(), pb.len()))
            }
        }

        /// Move the current pointer to the next pbuf in chain
        ///
        /// Returns:
        /// -1: no next part  
        ///  1: moved, now at last part  
        ///  0: moved, more parts remain
        pub fn next(&mut self) -> i8 {
            if let Some(ptr) = self.ptr {
                unsafe {
                    let pb = &mut *ptr;
                    if pb.next().is_none() {
                        return -1;
                    }
                    self.ptr = pb.next_mut();
                    if self.ptr.unwrap().as_ref().next().is_none() {
                        return 1;
                    }
                    return 0;
                }
            }
            -1
        }

        /// Move current pointer to the first pbuf
        pub fn first(&mut self) {
            if let Some(ref pb) = self.p {
                self.ptr = Some(&**pb as *const _ as *mut _);
            }
        }
    }
}
