//! Linux `splice(2)` & `vmsplice(2)` Zero-Copy Transport Engine
//!
//! Provides kernel-space zero-copy I/O streaming between memory buffers and Unix pipes.
//! Slices memory directly into pipe page ring buffers without intermediate user-space copies
//! or CPU cache thrashing. Includes graceful cross-platform fallbacks for non-Linux OSes.

use std::fs::File;
use std::io::{self, Error, ErrorKind};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

/// RAII wrapper managing Unix pipe descriptors for zero-copy streaming.
pub struct SplicePipe {
    reader: Option<File>,
    writer: Option<File>,
}

impl SplicePipe {
    /// Create a new OS pipe with close-on-exec (O_CLOEXEC) flags set.
    pub fn new() -> io::Result<Self> {
        let mut pipefd = [0 as libc::c_int; 2];
        #[cfg(target_os = "linux")]
        let res = unsafe {
            // SAFETY: pipefd is a valid, 2-element array of c_int allocated on the stack.
            // O_CLOEXEC ensures descriptors are not leaked across fork/exec boundaries.
            libc::pipe2(pipefd.as_mut_ptr(), libc::O_CLOEXEC)
        };
        #[cfg(not(target_os = "linux"))]
        let res = unsafe {
            // SAFETY: pipefd is a valid 2-element array of c_int.
            libc::pipe(pipefd.as_mut_ptr())
        };

        if res != 0 {
            return Err(Error::last_os_error());
        }

        // SAFETY: pipefd contains valid, newly created file descriptors returned by libc::pipe.
        // File::from_raw_fd assumes ownership and ensures descriptors are closed when dropped.
        let reader = unsafe { File::from_raw_fd(pipefd[0]) };
        let writer = unsafe { File::from_raw_fd(pipefd[1]) };

        Ok(Self {
            reader: Some(reader),
            writer: Some(writer),
        })
    }

    /// Mutable reference to the reader file handle.
    pub fn reader_mut(&mut self) -> Option<&mut File> {
        self.reader.as_mut()
    }

    /// Raw file descriptor of the reader end of the pipe.
    pub fn reader_raw_fd(&self) -> Option<RawFd> {
        self.reader.as_ref().map(|f| f.as_raw_fd())
    }

    /// Raw file descriptor of the writer end of the pipe.
    pub fn writer_raw_fd(&self) -> Option<RawFd> {
        self.writer.as_ref().map(|f| f.as_raw_fd())
    }

    /// Extract and take ownership of the reader handle.
    pub fn take_reader(&mut self) -> Option<File> {
        self.reader.take()
    }

    /// Explicitly close the writer end of the pipe to signal EOF to readers.
    pub fn close_writer(&mut self) {
        self.writer.take();
    }

    /// Explicitly close the reader end of the pipe.
    pub fn close_reader(&mut self) {
        self.reader.take();
    }
}

/// Zero-copy splice transport methods.
pub struct SpliceTransport;

impl SpliceTransport {
    /// Write memory buffer into a SplicePipe writer using Linux vmsplice zero-copy mapping.
    pub fn write_to_pipe(pipe: &mut SplicePipe, data: &[u8]) -> io::Result<usize> {
        let fd = pipe
            .writer_raw_fd()
            .ok_or_else(|| Error::new(ErrorKind::BrokenPipe, "Pipe writer closed"))?;
        Self::write_to_pipe_fd(fd, data)
    }

    /// Write memory buffer to any pipe descriptor using vmsplice with graceful write(2) fallback.
    pub fn write_to_pipe_fd(pipe_fd: RawFd, data: &[u8]) -> io::Result<usize> {
        #[cfg(target_os = "linux")]
        {
            vmsplice_write(pipe_fd, data)
        }
        #[cfg(not(target_os = "linux"))]
        {
            fallback_write(pipe_fd, data)
        }
    }

    /// Splice data directly between two kernel pipes without bouncing into user-space memory.
    pub fn splice_pipe_to_pipe(in_fd: RawFd, out_fd: RawFd, len: usize) -> io::Result<usize> {
        #[cfg(target_os = "linux")]
        {
            splice_kernel_pipes(in_fd, out_fd, len)
        }
        #[cfg(not(target_os = "linux"))]
        {
            fallback_pipe_to_pipe(in_fd, out_fd, len)
        }
    }
}

/// Linux zero-copy memory to pipe splicing via vmsplice(2).
#[cfg(target_os = "linux")]
fn vmsplice_write(pipe_fd: RawFd, mut data: &[u8]) -> io::Result<usize> {
    let mut total_written = 0;

    while !data.is_empty() {
        // vmsplice can transfer up to 1MB chunks cleanly
        let chunk_len = data.len().min(1024 * 1024);
        let iov = libc::iovec {
            iov_base: data.as_ptr() as *mut libc::c_void,
            iov_len: chunk_len,
        };

        // SAFETY:
        // 1. pipe_fd is a valid file descriptor.
        // 2. iov points to a valid slice of `data` guaranteed by Rust's slice lifetime.
        // 3. nr_segs is 1, matching the single iovec struct passed.
        // 4. SPLICE_F_GIFT requests page stealing if pages are aligned and whole, safe with standard allocations.
        let ret =
            unsafe { libc::vmsplice(pipe_fd, &iov as *const libc::iovec, 1, libc::SPLICE_F_GIFT) };

        if ret < 0 {
            let err = Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            // If vmsplice failed because the fd is not a pipe or unsupported (e.g. EINVAL),
            // seamlessly fall back to standard write(2) without failing the request.
            if total_written == 0 {
                return fallback_write(pipe_fd, data);
            }
            return Err(err);
        }

        let written = ret as usize;
        total_written += written;
        data = &data[written..];
    }

    Ok(total_written)
}

/// Linux in-kernel page transfer between two pipe descriptors via splice(2).
#[cfg(target_os = "linux")]
fn splice_kernel_pipes(in_fd: RawFd, out_fd: RawFd, mut len: usize) -> io::Result<usize> {
    let mut total_spliced = 0;

    while len > 0 {
        let chunk = len.min(1024 * 1024);

        // SAFETY:
        // 1. in_fd and out_fd are valid open file descriptors.
        // 2. NULL offsets indicate current descriptor positions are used.
        // 3. SPLICE_F_MOVE requests kernel page table remapping rather than data copy.
        let ret = unsafe {
            libc::splice(
                in_fd,
                std::ptr::null_mut(),
                out_fd,
                std::ptr::null_mut(),
                chunk,
                libc::SPLICE_F_MOVE | libc::SPLICE_F_MORE,
            )
        };

        if ret < 0 {
            let err = Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            if total_spliced == 0 {
                return fallback_pipe_to_pipe(in_fd, out_fd, len);
            }
            return Err(err);
        }

        let spliced = ret as usize;
        if spliced == 0 {
            break; // EOF reached on input pipe
        }

        total_spliced += spliced;
        len -= spliced;
    }

    Ok(total_spliced)
}

/// Portable POSIX write(2) fallback for non-Linux systems or non-pipe descriptors.
fn fallback_write(fd: RawFd, mut data: &[u8]) -> io::Result<usize> {
    let mut total = 0;
    while !data.is_empty() {
        // SAFETY: fd is valid and data slice is guaranteed valid for the specified length.
        let res = unsafe { libc::write(fd, data.as_ptr() as *const libc::c_void, data.len()) };
        if res < 0 {
            let err = Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(err);
        }
        let written = res as usize;
        total += written;
        data = &data[written..];
    }
    Ok(total)
}

/// Portable pipe-to-pipe copying fallback for non-Linux platforms.
fn fallback_pipe_to_pipe(in_fd: RawFd, out_fd: RawFd, len: usize) -> io::Result<usize> {
    let mut buf = vec![0u8; len.min(64 * 1024)];
    let mut remaining = len;
    let mut total = 0;

    while remaining > 0 {
        let to_read = remaining.min(buf.len());
        // SAFETY: in_fd is valid and buf has allocated capacity for to_read bytes.
        let res = unsafe { libc::read(in_fd, buf.as_mut_ptr() as *mut libc::c_void, to_read) };
        if res < 0 {
            let err = Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(err);
        }
        let n = res as usize;
        if n == 0 {
            break;
        }
        fallback_write(out_fd, &buf[..n])?;
        total += n;
        remaining -= n;
    }

    Ok(total)
}
