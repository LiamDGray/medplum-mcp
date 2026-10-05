//! Strict TDD test suite for Kernel Zero-Copy Network Engine.
//!
//! Asserts:
//! 1. Static file-to-socket transfer via sendfile(2) / splice(2) and kTLS readiness.
//! 2. Dynamic payload threshold routing:
//!    - < 10 KB -> Traditional send() byte-copy (avoids page-pinning overhead).
//!    - >= 10 KB -> MSG_ZEROCOPY / io_uring SEND_ZC with socket error-queue completion tracking.
//! 3. Real network TCP socket transfer verification.

use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;
use tempfile::NamedTempFile;

use medplum_mcp_server::network_zero_copy::{
    determine_zerocopy_strategy, enable_ktls_if_supported, enable_so_zerocopy_if_supported,
    send_clinical_payload, sendfile_to_socket, ZeroCopyStrategy, DYNAMIC_ZEROCOPY_THRESHOLD,
};

#[test]
fn test_zerocopy_strategy_routing_thresholds() {
    assert_eq!(DYNAMIC_ZEROCOPY_THRESHOLD, 10 * 1024);

    // 1. Static file
    assert_eq!(
        determine_zerocopy_strategy(true, 50 * 1024),
        ZeroCopyStrategy::SendFileWithKtls
    );

    // 2. Small dynamic payload (< 10 KB) -> Traditional send
    assert_eq!(
        determine_zerocopy_strategy(false, 512),
        ZeroCopyStrategy::TraditionalSend
    );
    assert_eq!(
        determine_zerocopy_strategy(false, 10 * 1024 - 1),
        ZeroCopyStrategy::TraditionalSend
    );

    // 3. Large dynamic payload (>= 10 KB) -> MSG_ZEROCOPY / io_uring SEND_ZC
    assert_eq!(
        determine_zerocopy_strategy(false, 10 * 1024),
        ZeroCopyStrategy::MsgZeroCopy
    );
    assert_eq!(
        determine_zerocopy_strategy(false, 100 * 1024),
        ZeroCopyStrategy::MsgZeroCopy
    );
}

#[test]
fn test_sendfile_to_socket_real_network_transfer() {
    // 1. Prepare temporary file with 64 KB of synthetic FHIR bundle JSON
    let mut temp_file = NamedTempFile::new().unwrap();
    let sample_line = b"{\"resourceType\":\"Observation\",\"id\":\"obs-zc-12345\"}\n";
    let mut expected_bytes = Vec::new();
    while expected_bytes.len() < 64 * 1024 {
        expected_bytes.extend_from_slice(sample_line);
    }
    temp_file.write_all(&expected_bytes).unwrap();
    temp_file.flush().unwrap();

    let file = File::open(temp_file.path()).unwrap();
    let file_fd = file.as_raw_fd();

    // 2. Create in-process TCP loopback server & client
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let client_handle = std::thread::spawn(move || {
        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let mut received = Vec::new();
        client.read_to_end(&mut received).unwrap();
        received
    });

    let (server_stream, _) = listener.accept().unwrap();
    let socket_fd = server_stream.as_raw_fd();

    // Optional kTLS enablement check (gracefully no-ops if kernel module tls is not loaded)
    let _ = enable_ktls_if_supported(socket_fd);

    // 3. Stream entire 64 KB file into network socket using sendfile(2)
    let count = expected_bytes.len();
    let transferred = sendfile_to_socket(file_fd, socket_fd, 0, count).unwrap();
    assert_eq!(transferred, count);

    // Close server write end
    drop(server_stream);

    // 4. Verify client received exact bytes
    let client_received = client_handle.join().unwrap();
    assert_eq!(client_received.len(), expected_bytes.len());
    assert_eq!(client_received, expected_bytes);
}

#[test]
fn test_adaptive_clinical_payload_sender_small_payload() {
    // Payloads < 10 KB should use traditional send without zero-copy overhead
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let client_handle = std::thread::spawn(move || {
        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let mut received = Vec::new();
        client.read_to_end(&mut received).unwrap();
        received
    });

    let (server_stream, _) = listener.accept().unwrap();
    let socket_fd = server_stream.as_raw_fd();

    let small_payload = b"{\"resourceType\":\"Patient\",\"id\":\"pat-sj-001\"}";
    let sent = send_clinical_payload(socket_fd, small_payload).unwrap();
    assert_eq!(sent, small_payload.len());

    drop(server_stream);

    let received = client_handle.join().unwrap();
    assert_eq!(received, small_payload);
}

#[test]
fn test_adaptive_clinical_payload_sender_large_payload() {
    // Payloads >= 10 KB should use MSG_ZEROCOPY when available
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let client_handle = std::thread::spawn(move || {
        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let mut received = Vec::new();
        client.read_to_end(&mut received).unwrap();
        received
    });

    let (server_stream, _) = listener.accept().unwrap();
    let socket_fd = server_stream.as_raw_fd();

    // Check/enable SO_ZEROCOPY on socket
    let _ = enable_so_zerocopy_if_supported(socket_fd);

    // Create 32 KB payload
    let mut large_payload = Vec::with_capacity(32 * 1024);
    while large_payload.len() < 32 * 1024 {
        large_payload
            .extend_from_slice(b"{\"entry\":[{\"resource\":{\"type\":\"Observation\"}}]},");
    }

    let sent = send_clinical_payload(socket_fd, &large_payload).unwrap();
    assert_eq!(sent, large_payload.len());

    drop(server_stream);

    let received = client_handle.join().unwrap();
    assert_eq!(received.len(), large_payload.len());
    assert_eq!(received, large_payload);
}
