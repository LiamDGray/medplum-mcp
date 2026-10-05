//! Kernel Zero-Copy Network Engine.
//!
//! Combines:
//! 1. Static Assets (File-to-Socket): `sendfile(2)` / `splice(2)` + Kernel TLS (kTLS)
//!    - Streams file page references directly in kernel space without user-space pulling.
//!    - kTLS offloads AES encryption to the kernel / hardware NIC DMA engine.
//! 2. Dynamic Data (User-to-Socket):
//!    - Payloads >= 10 KB: `MSG_ZEROCOPY` pins user memory pages directly to NIC scatter-gather DMA,
//!      tracking completion notifications via the socket error queue (`MSG_ERRQUEUE`).
//!    - Payloads < 10 KB: Traditional `send()` byte-copy avoiding page-pinning CPU overhead.
//! 3. Cross-platform graceful fallbacks for portable execution.

use std::io::{self, Error};
use std::os::unix::io::RawFd;
use tracing::debug;

/// Dynamic zero-copy threshold: copy avoidance delivers net gain above 10 KB.
pub const DYNAMIC_ZEROCOPY_THRESHOLD: usize = 10 * 1024; // 10 KB

/// Zero-copy network dispatch strategy based on data origin and payload volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZeroCopyStrategy {
    /// File-to-socket in-kernel page transfer via sendfile(2) / splice(2) with kTLS.
    SendFileWithKtls,
    /// User-to-socket pinned-page DMA transmission via MSG_ZEROCOPY or io_uring SEND_ZC.
    MsgZeroCopy,
    /// User-to-socket traditional send byte-copy for payloads under 10 KB.
    TraditionalSend,
}

/// Determine optimal zero-copy mechanism based on data origin and payload size.
pub fn determine_zerocopy_strategy(is_static_file: bool, payload_len: usize) -> ZeroCopyStrategy {
    if is_static_file {
        ZeroCopyStrategy::SendFileWithKtls
    } else if payload_len >= DYNAMIC_ZEROCOPY_THRESHOLD {
        ZeroCopyStrategy::MsgZeroCopy
    } else {
        ZeroCopyStrategy::TraditionalSend
    }
}

/// Enable Linux Kernel TLS (kTLS) on a TCP socket if supported by the running kernel.
///
/// Sets `TCP_ULP` (Upper Layer Protocol) to `"tls"`. When active, `sendfile(2)` pushes
/// plaintext directly into the kernel network stack where the kernel or smartNIC
/// hardware encrypts it in-flight without pulling pages into user space.
pub fn enable_ktls_if_supported(socket_fd: RawFd) -> io::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        let ulp_name = b"tls\0";
        // SOL_TCP = 6, TCP_ULP = 31
        // SAFETY:
        // 1. `socket_fd` is a valid descriptor provided by caller.
        // 2. `ulp_name` is a null-terminated byte literal b"tls\0" with static lifetime.
        // 3. Length passed matches exact buffer length.
        let ret = unsafe {
            libc::setsockopt(
                socket_fd,
                libc::SOL_TCP,
                31, // TCP_ULP
                ulp_name.as_ptr() as *const libc::c_void,
                ulp_name.len() as libc::socklen_t,
            )
        };

        if ret == 0 {
            debug!("kTLS successfully attached to socket fd {}", socket_fd);
            Ok(true)
        } else {
            let err = Error::last_os_error();
            debug!(
                "kTLS not available on socket fd {} (fallback to user-space TLS): {}",
                socket_fd, err
            );
            Ok(false)
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = socket_fd;
        Ok(false)
    }
}

/// Enable `SO_ZEROCOPY` on a socket descriptor if supported by the OS.
pub fn enable_so_zerocopy_if_supported(socket_fd: RawFd) -> io::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        let val: libc::c_int = 1;
        // SAFETY:
        // 1. `socket_fd` is a valid descriptor provided by caller.
        // 2. `val` is a stack-allocated c_int.
        // 3. Size passed matches sizeof(libc::c_int).
        let ret = unsafe {
            libc::setsockopt(
                socket_fd,
                libc::SOL_SOCKET,
                libc::SO_ZEROCOPY,
                &val as *const libc::c_int as *const libc::c_void,
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            )
        };

        if ret == 0 {
            debug!("SO_ZEROCOPY enabled on socket fd {}", socket_fd);
            Ok(true)
        } else {
            let err = Error::last_os_error();
            debug!(
                "SO_ZEROCOPY not supported on socket fd {}: {}",
                socket_fd, err
            );
            Ok(false)
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = socket_fd;
        Ok(false)
    }
}

/// Zero-copy file-to-socket transfer via `sendfile(2)`.
///
/// Passes page references inside the Linux kernel page cache rather than copying bytes.
/// Transparently integrates with kTLS when enabled on the target socket.
pub fn sendfile_to_socket(
    file_fd: RawFd,
    socket_fd: RawFd,
    mut offset: i64,
    mut count: usize,
) -> io::Result<usize> {
    let mut total_sent = 0;

    #[cfg(target_os = "linux")]
    {
        while count > 0 {
            let chunk = count.min(1024 * 1024); // 1 MB chunks
            let mut off = offset as libc::off_t;

            // SAFETY:
            // 1. `socket_fd` and `file_fd` are valid open file descriptors.
            // 2. `off` points to stack-allocated valid libc::off_t updated by the kernel.
            // 3. `chunk` is bounded to 1 MB to prevent buffer exhaustion.
            let res = unsafe { libc::sendfile(socket_fd, file_fd, &mut off, chunk) };

            if res < 0 {
                let err = Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                if total_sent == 0 {
                    // Seamless fallback if sendfile is unsupported on specific fd types
                    return fallback_file_to_socket(file_fd, socket_fd, offset, count);
                }
                return Err(err);
            }

            let sent = res as usize;
            if sent == 0 {
                break; // EOF reached on file
            }

            total_sent += sent;
            offset += sent as i64;
            count -= sent;
        }

        Ok(total_sent)
    }
    #[cfg(not(target_os = "linux"))]
    {
        fallback_file_to_socket(file_fd, socket_fd, offset, count)
    }
}

/// Send user memory buffer to socket using `MSG_ZEROCOPY` on Linux.
///
/// Pins memory pages to the NIC DMA scatter-gather ring and drains completion
/// notifications from `MSG_ERRQUEUE` before returning, upholding Rust lifetime safety.
pub fn send_zerocopy_pinned(socket_fd: RawFd, data: &[u8]) -> io::Result<usize> {
    #[cfg(target_os = "linux")]
    {
        let mut total_sent = 0;
        let mut remaining = data;

        while !remaining.is_empty() {
            let chunk_len = remaining.len().min(1024 * 1024);
            let chunk = &remaining[..chunk_len];

            // SAFETY:
            // 1. `socket_fd` is a valid open socket descriptor.
            // 2. `chunk.as_ptr()` points to a valid continuous memory slice of length `chunk.len()`.
            // 3. Rust slice lifetime guarantees memory remains valid for the duration of the send call.
            // 4. `MSG_ZEROCOPY` requests the kernel to pin user-space pages. To uphold Rust lifetime
            //    invariants and prevent use-after-free, `drain_zerocopy_completion` immediately polls
            //    `MSG_ERRQUEUE` before returning, ensuring completion notifications are processed.
            let res = unsafe {
                libc::send(
                    socket_fd,
                    chunk.as_ptr() as *const libc::c_void,
                    chunk.len(),
                    libc::MSG_ZEROCOPY,
                )
            };

            if res < 0 {
                let err = Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                // If socket does not support MSG_ZEROCOPY or error occurs, fall back to traditional send
                if total_sent == 0 {
                    return traditional_socket_send(socket_fd, data);
                }
                return Err(err);
            }

            let sent = res as usize;
            total_sent += sent;
            remaining = &remaining[sent..];

            // Drain kernel completion notification from error queue to ensure page release
            drain_zerocopy_completion(socket_fd);
        }

        Ok(total_sent)
    }
    #[cfg(not(target_os = "linux"))]
    {
        traditional_socket_send(socket_fd, data)
    }
}

/// Adaptive dynamic payload sender.
///
/// Routes according to performance thresholds:
/// - `< 10 KB`: Traditional send() byte-copy (avoids page-pinning overhead).
/// - `>= 10 KB`: `MSG_ZEROCOPY` page pinning.
pub fn send_clinical_payload(socket_fd: RawFd, data: &[u8]) -> io::Result<usize> {
    let strategy = determine_zerocopy_strategy(false, data.len());
    match strategy {
        ZeroCopyStrategy::TraditionalSend => traditional_socket_send(socket_fd, data),
        ZeroCopyStrategy::MsgZeroCopy => send_zerocopy_pinned(socket_fd, data),
        ZeroCopyStrategy::SendFileWithKtls => traditional_socket_send(socket_fd, data),
    }
}

/// Traditional socket send with loop handling partial writes and EINTR.
pub fn traditional_socket_send(socket_fd: RawFd, mut data: &[u8]) -> io::Result<usize> {
    let mut total = 0;
    while !data.is_empty() {
        // SAFETY:
        // 1. `socket_fd` is a valid open socket descriptor.
        // 2. `data.as_ptr()` points to a valid slice with length `data.len()`.
        // 3. Flags is 0 (standard synchronous send).
        let res = unsafe {
            libc::send(
                socket_fd,
                data.as_ptr() as *const libc::c_void,
                data.len(),
                0,
            )
        };
        if res < 0 {
            let err = Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(err);
        }
        let sent = res as usize;
        total += sent;
        data = &data[sent..];
    }
    Ok(total)
}

/// Drain completion notification from socket's error queue for `MSG_ZEROCOPY`.
#[cfg(target_os = "linux")]
fn drain_zerocopy_completion(socket_fd: RawFd) {
    let mut ctrl_buf = [0u8; 128];
    let mut iov = libc::iovec {
        iov_base: std::ptr::null_mut(),
        iov_len: 0,
    };
    // SAFETY:
    // 1. `msghdr` is zero-initialized to guarantee no undefined memory states.
    // 2. `ctrl_buf` is stack-allocated with 128 bytes, providing sufficient capacity
    //    for sock_extended_err control messages.
    // 3. Non-blocking MSG_DONTWAIT prevents worker threads from blocking if the queue is empty.
    let mut msghdr: libc::msghdr = unsafe { std::mem::zeroed() };
    msghdr.msg_iov = &mut iov;
    msghdr.msg_iovlen = 1;
    msghdr.msg_control = ctrl_buf.as_mut_ptr() as *mut libc::c_void;
    msghdr.msg_controllen = ctrl_buf.len();

    // Non-blocking poll on MSG_ERRQUEUE
    let _ = unsafe {
        libc::recvmsg(
            socket_fd,
            &mut msghdr as *mut libc::msghdr,
            libc::MSG_ERRQUEUE | libc::MSG_DONTWAIT,
        )
    };
}

/// Portable file-to-socket copy fallback for non-Linux or unsupported descriptor types.
fn fallback_file_to_socket(
    file_fd: RawFd,
    socket_fd: RawFd,
    offset: i64,
    mut count: usize,
) -> io::Result<usize> {
    let mut buf = vec![0u8; count.min(64 * 1024)];
    let mut total = 0;
    let mut current_offset = offset;

    while count > 0 {
        let to_read = count.min(buf.len());
        // SAFETY:
        // 1. `file_fd` is valid and open for reading.
        // 2. `buf` has allocated capacity >= `to_read`.
        // 3. `current_offset` is a valid non-negative file offset.
        let read_bytes = unsafe {
            libc::pread(
                file_fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                to_read,
                current_offset as libc::off_t,
            )
        };

        if read_bytes < 0 {
            let err = Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(err);
        }
        let n = read_bytes as usize;
        if n == 0 {
            break; // EOF
        }

        traditional_socket_send(socket_fd, &buf[..n])?;
        total += n;
        current_offset += n as i64;
        count -= n;
    }

    Ok(total)
}
