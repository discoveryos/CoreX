//! Async Rust SLIP interface

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

const SLIP_END: u8 = 0xC0;
const SLIP_ESC: u8 = 0xDB;
const SLIP_ESC_END: u8 = 0xDC;
const SLIP_ESC_ESC: u8 = 0xDD;

/// Async SLIP interface using a Tokio AsyncRead + AsyncWrite serial stream
pub struct AsyncSlipif<S> {
    serial: S,
    rx_buffer: Vec<u8>,
}

impl<S> AsyncSlipif<S>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    /// Create a new async SLIP interface
    pub fn new(serial: S) -> Self {
        Self {
            serial,
            rx_buffer: Vec::new(),
        }
    }

    /// Send a full packet
    pub async fn send(&mut self, packet: &[u8]) -> tokio::io::Result<()> {
        self.serial.write_all(&[SLIP_END]).await?;
        for &b in packet {
            match b {
                SLIP_END => self.serial.write_all(&[SLIP_ESC, SLIP_ESC_END]).await?,
                SLIP_ESC => self.serial.write_all(&[SLIP_ESC, SLIP_ESC_ESC]).await?,
                _ => self.serial.write_all(&[b]).await?,
            }
        }
        self.serial.write_all(&[SLIP_END]).await?;
        self.serial.flush().await
    }

    /// Read the next packet from the serial interface
    pub async fn next_packet(&mut self) -> tokio::io::Result<Vec<u8>> {
        let mut buf = [0u8; 1];
        loop {
            let n = self.serial.read(&mut buf).await?;
            if n == 0 {
                continue; // EOF or no data yet
            }

            let byte = buf[0];

            match byte {
                SLIP_END => {
                    if !self.rx_buffer.is_empty() {
                        let packet = self.rx_buffer.clone();
                        self.rx_buffer.clear();
                        return Ok(packet);
                    }
                }
                SLIP_ESC => {
                    // read next byte for escape
                    let n = self.serial.read(&mut buf).await?;
                    if n == 0 {
                        continue; // incomplete, try again
                    }
                    match buf[0] {
                        SLIP_ESC_END => self.rx_buffer.push(SLIP_END),
                        SLIP_ESC_ESC => self.rx_buffer.push(SLIP_ESC),
                        other => self.rx_buffer.push(other), // protocol error fallback
                    }
                }
                _ => self.rx_buffer.push(byte),
            }
        }
    }

    /// Spawn a task that continuously reads packets and sends them via channel
    pub fn spawn_reader(
        mut self,
    ) -> mpsc::Receiver<Vec<u8>> {
        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            loop {
                match self.next_packet().await {
                    Ok(pkt) => {
                        if tx.send(pkt).await.is_err() {
                            break; // receiver dropped
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        rx
    }
}
