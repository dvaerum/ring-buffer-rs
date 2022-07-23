use std::ptr::copy_nonoverlapping;
use std::{io, mem};
use std::cmp::{max, min};
use std::io::Write;

#[cfg(feature = "async")]
use futures_io::AsyncWrite;
#[cfg(feature = "async")]
use std::{pin::Pin, task::{Context, Poll}};

use super::RingBuffer;
use super::BufferType;

#[derive(Debug)]
pub struct RingWriter {
    pub(crate) buffer: BufferType<RingBuffer>,
}

#[cfg(any(test, debug))]
fn check_for_buffer_overflow(buf: &Vec<u8>) {
    let buf_ptr = (*buf).as_ptr();
    for offset in buf.len()..buf.capacity() {
        unsafe {
            let v = buf_ptr.offset(offset as isize).read();
            if v != 0 {
                panic!("IMPOSSIBLE: Buffer overflow detected - offset: {} - value: {}",
                       offset, v)
            }
        }
    }
}

impl Write for RingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut write_count: usize = 0;

        if self.buffer.current_writer_location == self.buffer.current_reader_location {
            if let Ok(mut buffer_inner) = self.buffer.inner.try_lock() {
                let space_left = buffer_inner.len() - 1;
                write_count = min(space_left, buf.len());
                let leftover = if self.buffer.current_writer_location + write_count > buffer_inner.len() {
                    (self.buffer.current_writer_location + write_count) - buffer_inner.len()
                } else {
                    0
                };
                unsafe {
                    copy_nonoverlapping(
                        buf.as_ptr(),
                        buffer_inner.as_mut_ptr().offset(self.buffer.current_writer_location as isize),
                        write_count - max(leftover, 0),
                    );
                }
                if leftover > 0 {
                    unsafe {
                        copy_nonoverlapping(
                            buf.as_ptr().offset((write_count - leftover) as isize),
                            buffer_inner.as_mut_ptr(),
                            leftover,
                        );
                    }
                }
                #[cfg(any(test, debug))]
                check_for_buffer_overflow(&buffer_inner);
            }
        } else if self.buffer.current_writer_location > self.buffer.current_reader_location {
            if let Ok(mut buffer_inner) = self.buffer.inner.try_lock() {
                let space_left = (buffer_inner.len() - self.buffer.current_writer_location) + self.buffer.current_reader_location;
                write_count = min(space_left, buf.len());
                let leftover = (self.buffer.current_writer_location + write_count) - buffer_inner.len();
                unsafe {
                    copy_nonoverlapping(
                        buf.as_ptr(),
                        buffer_inner.as_mut_ptr().offset(self.buffer.current_writer_location as isize),
                        write_count - max(leftover, 0),
                    );
                }
                if leftover > 0 {
                    unsafe {
                        copy_nonoverlapping(
                            buf.as_ptr().offset((write_count - max(leftover, 0)) as isize),
                            buffer_inner.as_mut_ptr(),
                            leftover,
                        );
                    }
                }
                #[cfg(any(test, debug))]
                check_for_buffer_overflow(&buffer_inner);
            }
        } else if self.buffer.current_writer_location < self.buffer.current_reader_location {
            if let Ok(mut buffer_inner) = self.buffer.inner.try_lock() {
                let space_left = (self.buffer.current_reader_location - 1) - self.buffer.current_writer_location;
                write_count = min(space_left, buf.len());
                unsafe {
                    copy_nonoverlapping(
                        buf.as_ptr(),
                        buffer_inner.as_mut_ptr().offset(self.buffer.current_writer_location as isize),
                        write_count,
                    );
                }
                #[cfg(any(test, debug))]
                check_for_buffer_overflow(&buffer_inner);
            }
        } else {
            unimplemented!(
                "RingWriter: current_writer_location: {} - current_reader_location: {}",
                self.buffer.current_writer_location, self.buffer.current_reader_location)
        }

        self.buffer.add_to_current_writer_location(write_count);
        return Ok(write_count);
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl RingWriter {
    fn _close(&self) {
        unsafe {
            let ptr: *mut bool = mem::transmute(&self.buffer.writer_closed);
            *ptr = true
        }
    }
}

impl Drop for RingWriter {
    fn drop(&mut self) {
        self._close()
    }
}

#[cfg(feature = "async")]
impl AsyncWrite for RingWriter {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let r = self.get_mut().write(buf);
        Poll::Ready(r)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        self._close();
        Poll::Ready(Ok(()))
    }
}
