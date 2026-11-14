use super::*;
use std::net::TcpListener;

/// A one-connection server: answers the handshake, reads one client frame,
/// then sends back its text split in two frames with a ping in between.
fn echo_server() -> (u16, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        assert!(String::from_utf8_lossy(&head).contains("Upgrade: websocket"));
        stream
            .write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n")
            .unwrap();
        let mut frame = [0_u8; 2];
        stream.read_exact(&mut frame).unwrap();
        assert_eq!(frame[0], 0x81, "a final text frame");
        assert!(frame[1] & 0x80 != 0, "a client frame is masked");
        let length = usize::from(frame[1] & 0x7F);
        let mut key = [0_u8; 4];
        stream.read_exact(&mut key).unwrap();
        let mut payload = vec![0_u8; length];
        stream.read_exact(&mut payload).unwrap();
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= key[index % 4];
        }
        let text = String::from_utf8(payload).unwrap();
        let (first, second) = text.split_at(text.len() / 2);
        let mut reply = vec![0x01, first.len() as u8];
        reply.extend_from_slice(first.as_bytes());
        reply.extend_from_slice(&[0x89, 0x00]);
        reply.extend_from_slice(&[0x80, second.len() as u8]);
        reply.extend_from_slice(second.as_bytes());
        stream.write_all(&reply).unwrap();
        let mut pong = [0_u8; 6];
        stream.read_exact(&mut pong).unwrap();
        assert_eq!(pong[0], 0x8A, "the ping is answered");
        text
    });
    (port, handle)
}

#[test]
fn a_text_message_goes_out_masked_and_comes_back_reassembled() {
    let (port, server) = echo_server();
    let url = format!("ws://127.0.0.1:{port}/devtools/page/1");
    let mut socket = Socket::connect(&url, Duration::from_secs(5)).expect("connects");
    assert!(socket.send_text("{\"id\":1,\"method\":\"Runtime.evaluate\"}"));
    let reply = socket.read_text(Instant::now() + Duration::from_secs(5));
    assert_eq!(
        reply.as_deref(),
        Some("{\"id\":1,\"method\":\"Runtime.evaluate\"}")
    );
    assert_eq!(
        server.join().unwrap(),
        "{\"id\":1,\"method\":\"Runtime.evaluate\"}"
    );
}

#[test]
fn only_a_loopback_address_is_connected_to() {
    assert!(
        Socket::connect(
            "ws://example.com:9222/devtools/page/1",
            Duration::from_millis(50)
        )
        .is_none()
    );
    assert!(
        Socket::connect(
            "wss://127.0.0.1:9222/devtools/page/1",
            Duration::from_millis(50)
        )
        .is_none()
    );
    assert!(Socket::connect("ws://127.0.0.1/devtools/page/1", Duration::from_millis(50)).is_none());
}
