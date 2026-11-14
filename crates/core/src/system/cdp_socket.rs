//! A minimal WebSocket client for a browser's DevTools endpoint on the
//! loopback interface: text messages only, no extensions, no TLS. It exists
//! so a page can be acted on without its window ever being activated.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_HANDSHAKE_BYTES: usize = 8 * 1024;
/// Any 16 bytes, base64: the server's answer to it is not checked, because
/// the peer is a loopback DevTools endpoint probed over HTTP just before.
const HANDSHAKE_KEY: &str = "YWdlbnQtZGVza3RvcC1jZA==";
const TEXT: u8 = 0x1;
const CONTINUATION: u8 = 0x0;
const CLOSE: u8 = 0x8;
const PING: u8 = 0x9;
const PONG: u8 = 0xA;

pub(crate) struct Socket {
    stream: TcpStream,
    mask: u32,
}

impl Socket {
    /// Connects to `ws://127.0.0.1:PORT/PATH`. Any other host is refused.
    pub(crate) fn connect(url: &str, timeout: Duration) -> Option<Self> {
        let (authority, path) = url.strip_prefix("ws://")?.split_once('/')?;
        let (host, port) = authority.rsplit_once(':')?;
        if host != "127.0.0.1" && host != "localhost" {
            return None;
        }
        let port: u16 = port.parse().ok()?;
        let address = SocketAddr::from(([127, 0, 0, 1], port));
        let mut stream = TcpStream::connect_timeout(&address, timeout).ok()?;
        stream.set_read_timeout(Some(timeout)).ok()?;
        stream.set_write_timeout(Some(timeout)).ok()?;
        let _ = stream.set_nodelay(true);
        let request = format!(
            "GET /{path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\n\
             Connection: Upgrade\r\nSec-WebSocket-Key: {HANDSHAKE_KEY}\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).ok()?;
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            if head.len() > MAX_HANDSHAKE_BYTES {
                return None;
            }
            stream.read_exact(&mut byte).ok()?;
            head.push(byte[0]);
        }
        if !head.starts_with(b"HTTP/1.1 101") {
            return None;
        }
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0x9E37_79B9, |since| since.subsec_nanos());
        Some(Self {
            stream,
            mask: seed | 1,
        })
    }

    pub(crate) fn send_text(&mut self, text: &str) -> bool {
        self.send_frame(TEXT, text.as_bytes())
    }

    /// The next whole text message, or `None` when the peer closes, sends
    /// something malformed, or nothing arrives before `until`.
    pub(crate) fn read_text(&mut self, until: Instant) -> Option<String> {
        let mut message = Vec::new();
        loop {
            let remaining = until.checked_duration_since(Instant::now())?;
            if remaining.is_zero() {
                return None;
            }
            self.stream.set_read_timeout(Some(remaining)).ok()?;
            let mut head = [0_u8; 2];
            self.stream.read_exact(&mut head).ok()?;
            let finished = head[0] & 0x80 != 0;
            let opcode = head[0] & 0x0F;
            let masked = head[1] & 0x80 != 0;
            let length = match head[1] & 0x7F {
                126 => {
                    let mut bytes = [0_u8; 2];
                    self.stream.read_exact(&mut bytes).ok()?;
                    u64::from(u16::from_be_bytes(bytes))
                }
                127 => {
                    let mut bytes = [0_u8; 8];
                    self.stream.read_exact(&mut bytes).ok()?;
                    u64::from_be_bytes(bytes)
                }
                short => u64::from(short),
            };
            let length = usize::try_from(length).ok()?;
            if length > MAX_MESSAGE_BYTES || message.len() + length > MAX_MESSAGE_BYTES {
                return None;
            }
            let mut key = [0_u8; 4];
            if masked {
                self.stream.read_exact(&mut key).ok()?;
            }
            let mut payload = vec![0_u8; length];
            self.stream.read_exact(&mut payload).ok()?;
            if masked {
                payload
                    .iter_mut()
                    .enumerate()
                    .for_each(|(index, byte)| *byte ^= key[index % 4]);
            }
            match opcode {
                TEXT | CONTINUATION => {
                    message.extend_from_slice(&payload);
                    if finished {
                        return String::from_utf8(message).ok();
                    }
                }
                CLOSE => return None,
                PING => {
                    if !self.send_frame(PONG, &payload) {
                        return None;
                    }
                }
                _ => {}
            }
        }
    }

    /// A client frame is always masked (RFC 6455, section 5.3).
    fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> bool {
        let mut frame = Vec::with_capacity(payload.len() + 14);
        frame.push(0x80 | opcode);
        match payload.len() {
            length if length < 126 => frame.push(0x80 | length as u8),
            length if length <= usize::from(u16::MAX) => {
                frame.push(0x80 | 126);
                frame.extend_from_slice(&(length as u16).to_be_bytes());
            }
            length => {
                frame.push(0x80 | 127);
                frame.extend_from_slice(&(length as u64).to_be_bytes());
            }
        }
        self.mask ^= self.mask << 13;
        self.mask ^= self.mask >> 17;
        self.mask ^= self.mask << 5;
        let key = self.mask.to_be_bytes();
        frame.extend_from_slice(&key);
        frame.extend(
            payload
                .iter()
                .enumerate()
                .map(|(index, byte)| byte ^ key[index % 4]),
        );
        self.stream.write_all(&frame).is_ok()
    }
}

#[cfg(test)]
#[path = "cdp_socket_tests.rs"]
mod tests;
