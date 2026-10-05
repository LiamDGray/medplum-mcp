use medplum_mcp_server::splice_transport::{SplicePipe, SpliceTransport};
use std::io::Read;

#[test]
fn test_splice_pipe_transport_write_and_read() {
    let mut pipe = SplicePipe::new().expect("creating splice pipe must succeed");
    let test_payload =
        b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{\"text\":\"FHIR stream\"}}\n";

    let bytes_written = SpliceTransport::write_to_pipe(&mut pipe, test_payload)
        .expect("zero-copy splice write must succeed");

    assert_eq!(bytes_written, test_payload.len());

    let mut read_buf = vec![0u8; test_payload.len()];
    pipe.reader_mut()
        .expect("reader available")
        .read_exact(&mut read_buf)
        .expect("reading back from splice pipe reader must succeed");

    assert_eq!(read_buf, test_payload);
}

#[test]
fn test_splice_pipe_to_pipe_transfer() {
    let mut pipe1 = SplicePipe::new().expect("creating pipe 1 must succeed");
    let mut pipe2 = SplicePipe::new().expect("creating pipe 2 must succeed");

    let test_data = b"zero-copy in-kernel pipe splice test payload";
    SpliceTransport::write_to_pipe(&mut pipe1, test_data).expect("write to pipe1");

    // Zero-copy kernel splice from pipe1 reader to pipe2 writer
    let spliced = SpliceTransport::splice_pipe_to_pipe(
        pipe1.reader_raw_fd().expect("reader fd"),
        pipe2.writer_raw_fd().expect("writer fd"),
        test_data.len(),
    )
    .expect("splicing between pipes in kernel must succeed");

    assert_eq!(spliced, test_data.len());

    let mut read_buf = vec![0u8; test_data.len()];
    pipe2
        .reader_mut()
        .expect("reader available")
        .read_exact(&mut read_buf)
        .expect("reading from pipe2 reader");

    assert_eq!(read_buf, test_data);
}

#[test]
fn test_splice_large_buffer_chunking() {
    let mut pipe = SplicePipe::new().expect("creating splice pipe must succeed");
    // 128KB payload to test chunking across Linux default pipe buffer sizes (usually 64KB)
    let large_payload = vec![0x42u8; 128 * 1024];

    let mut reader = pipe.take_reader().expect("take reader");
    let handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).expect("read to end");
        buf
    });

    let bytes_written =
        SpliceTransport::write_to_pipe_fd(pipe.writer_raw_fd().expect("writer fd"), &large_payload)
            .expect("splicing large payload must succeed");

    assert_eq!(bytes_written, large_payload.len());
    pipe.close_writer();

    let received = handle.join().expect("join reader thread");
    assert_eq!(received.len(), large_payload.len());
    assert_eq!(received, large_payload);
}
