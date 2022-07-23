use std::ptr::copy_nonoverlapping;
use std::io;
use std::cmp::{max, min};
use std::io::Read;

#[cfg(feature = "async")]
use futures_io::AsyncRead;
#[cfg(feature = "async")]
use std::{pin::Pin, task::{Context, Poll}};

use super::RingBuffer;
use super::BufferType;

#[derive(Debug)]
pub struct RingReader {
    pub(crate) buffer: BufferType<RingBuffer>,
}

impl RingReader {
    pub fn no_more_to_read(&self) -> bool {
        self.buffer.current_writer_location == self.buffer.current_reader_location && self.buffer.writer_closed
    }

    pub fn more_to_read(&self) -> bool {
        !self.no_more_to_read()
    }
}

impl Read for RingReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut read_count: usize = 0;

        if self.buffer.current_writer_location == self.buffer.current_reader_location {
            if self.buffer.writer_closed {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            };
        } else if self.buffer.current_writer_location < self.buffer.current_reader_location {
            if let Ok(ref mut buffer_inner) = self.buffer.inner.try_lock() {
                let space_left = (buffer_inner.len() - self.buffer.current_reader_location) + self.buffer.current_writer_location;
                read_count = min(space_left, buf.len());
                let leftover = (self.buffer.current_reader_location + read_count) - buffer_inner.len();
                unsafe {
                    copy_nonoverlapping(
                        buffer_inner.as_mut_ptr().offset(self.buffer.current_reader_location as isize),
                        buf.as_ptr() as *mut u8,
                        read_count - max(leftover, 0),
                    );
                }
                if leftover > 0 {
                    unsafe {
                        copy_nonoverlapping(
                            buffer_inner.as_mut_ptr(),
                            buf.as_ptr().offset((read_count - max(leftover, 0)) as isize) as *mut u8,
                            leftover,
                        );
                    }
                }
            }
        } else if self.buffer.current_writer_location > self.buffer.current_reader_location {
            if let Ok(ref mut buffer_inner) = self.buffer.inner.try_lock() {
                let space_left = self.buffer.current_writer_location - self.buffer.current_reader_location;
                read_count = min(space_left, buf.len());
                unsafe {
                    copy_nonoverlapping(
                        buffer_inner.as_mut_ptr().offset(self.buffer.current_reader_location as isize),
                        buf.as_ptr() as *mut u8,
                        read_count,
                    );
                }
            }
        } else {
            unimplemented!("Unknown state for RingReader")
        }

        self.buffer.add_to_current_reader_location(read_count);
        return Ok(read_count);
    }
}

#[cfg(feature = "async")]
impl AsyncRead for RingReader {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let r = self.get_mut().read(buf);
        Poll::Ready(r)
    }
}


