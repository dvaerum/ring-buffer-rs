mod writer;
mod reader;

use writer::RingWriter;
use reader::RingReader;

use std::{mem};
use std::sync::{Arc, Mutex};

type BufferType<T> = Arc<Box<T>>;

#[derive(Debug)]
pub struct RingBuffer {
    inner: Mutex<Vec<u8>>,
    current_writer_location: usize,
    current_reader_location: usize,
    writer_closed: bool,
}

impl RingBuffer {
    pub fn new(
        ring_buffer_size: usize,
    ) -> (RingWriter, RingReader) {
        #[cfg(not(any(test, debug)))]
        let mut buffer = Vec::with_capacity(ring_buffer_size);

        // Make a 100 bytes buffer zone to detect buffer overflow
        #[cfg(any(test, debug))]
        let mut buffer = Vec::with_capacity(ring_buffer_size + 100);

        unsafe {
            buffer.set_len(ring_buffer_size);

            let ptr: *mut u8 = buffer.as_mut_ptr();
            ptr.write_bytes(0u8, buffer.capacity())
        }

        let pipe_buffer = RingBuffer {
            inner: Mutex::new(buffer),
            current_writer_location: 0,
            current_reader_location: 0,
            writer_closed: false,
        };

        let arc_pipe_buffer = Arc::new(Box::new(pipe_buffer));

        let pipe_reader = RingReader {
            buffer: arc_pipe_buffer.clone(),
        };

        let pipe_writer = RingWriter {
            buffer: arc_pipe_buffer,
        };

        return (pipe_writer, pipe_reader);
    }

    fn add_to_current_writer_location(&self, bytes: usize) {
        if self.current_writer_location >= self.current_reader_location {
            if let Ok(buffer_inner) = self.inner.try_lock() {
                if self.current_writer_location + bytes > buffer_inner.len() {
                    if self.current_writer_location + bytes - buffer_inner.len() >= self.current_reader_location {
                        panic!("IMPOSSIBLE: The `current_writer_location`, is not allowed to become the same or over come the `current_reader_location`")
                    } else {
                        unsafe {
                            let ptr: *mut usize = mem::transmute(&self.current_writer_location);
                            *ptr = self.current_writer_location + bytes - buffer_inner.len();
                        }
                    }
                } else {
                    unsafe {
                        let ptr: *mut usize = mem::transmute(&self.current_writer_location);
                        *ptr = self.current_writer_location + bytes;
                    }
                }
            }
        } else {
            if self.current_writer_location + bytes >= self.current_reader_location {
                panic!("IMPOSSIBLE: The `current_writer_location`, \
                is not allowed to become the same or over come the `current_reader_location`")
            } else {
                unsafe {
                    let ptr: *mut usize = mem::transmute(&self.current_writer_location);
                    *ptr = self.current_writer_location + bytes;
                }
            }
        }
    }

    fn add_to_current_reader_location(&self, bytes: usize) {
        if self.current_reader_location > self.current_writer_location {
            if let Ok(buffer_inner) = self.inner.try_lock() {
                if self.current_reader_location + bytes > buffer_inner.len() {
                    if self.current_reader_location + bytes - buffer_inner.len() > self.current_writer_location {
                        panic!("IMPOSSIBLE: The `current_reader_location`, is not allowed to become the same or over come the `current_reader_location`")
                    } else {
                        unsafe {
                            let ptr: *mut usize = mem::transmute(&self.current_reader_location);
                            *ptr = self.current_reader_location + bytes - buffer_inner.len();
                        }
                    }
                } else {
                    unsafe {
                        let ptr: *mut usize = mem::transmute(&self.current_reader_location);
                        *ptr = self.current_reader_location + bytes;
                    }
                }
            }
        } else {
            if self.current_reader_location + bytes > self.current_writer_location {
                panic!("IMPOSSIBLE: The `current_reader_location`, is not allowed to become the same or over come the `current_reader_location`")
            } else {
                unsafe {
                    let ptr: *mut usize = mem::transmute(&self.current_reader_location);
                    *ptr = self.current_reader_location + bytes;
                }
            }
        }
    }
}

#[cfg(test)]
mod ring_buffer_tests {
    use std::io::{Read, Write};
    use super::RingBuffer;

    #[test]
    fn create_ring_buffer_only_write() {
        let (mut writer, reader) = RingBuffer::new(10);

        assert_eq!(6, writer.write(&[0, 1, 2, 3, 4, 5]).unwrap());
        assert_eq!(
            &[0u8, 1, 2, 3, 4, 5, 0, 0, 0, 0],
            writer.buffer.inner.try_lock().unwrap().as_slice()
        );
        println!("{:?}", writer.buffer);

        assert_eq!(4, writer.write(&[6, 7, 8, 9, 0, 1]).unwrap());
        assert_eq!(
            &[0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            writer.buffer.inner.try_lock().unwrap().as_slice()
        );
        println!("{:?}", writer.buffer);

        assert_eq!(0, writer.write(&[0, 1, 2, 3, 4, 5]).unwrap());
        assert_eq!(
            &[0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            writer.buffer.inner.try_lock().unwrap().as_slice()
        );
        println!("{:?}", writer.buffer);

        drop(writer);
        assert_eq!(true, reader.buffer.writer_closed);
        println!("{:?}", reader.buffer);
    }

    #[test]
    fn create_ring_buffer_write_and_read() {
        let (mut writer, mut reader) = RingBuffer::new(10);

        let mut buf = [0u8; 5];
        assert_eq!(0, reader.read(&mut buf).unwrap());
        assert_eq!(
            &[0, 0, 0, 0, 0],
            &buf
        );
        assert_eq!(0, reader.buffer.current_reader_location);
        println!("{:?}", writer.buffer);

        assert_eq!(6, writer.write(&[0, 1, 2, 3, 4, 5]).unwrap());
        assert_eq!(
            &[0, 1, 2, 3, 4, 5, 0, 0, 0, 0],
            reader.buffer.inner.try_lock().unwrap().as_slice()
        );
        println!("{:?}", writer.buffer);

        let mut buf = [0u8; 3];
        assert_eq!(3, reader.read(buf.as_mut()).unwrap());
        assert_eq!(
            &[0, 1, 2],
            &buf
        );
        assert_eq!(3, reader.buffer.current_reader_location);
        println!("{:?}", writer.buffer);

        assert_eq!(6, writer.write(&[6, 7, 8, 9, 10, 11]).unwrap());
        assert_eq!(
            &[10, 11, 2, 3, 4, 5, 6, 7, 8, 9],
            reader.buffer.inner.try_lock().unwrap().as_slice()
        );
        println!("{:?}", writer.buffer);

        assert_eq!(0, writer.write(&[12, 13, 14, 15, 16, 17]).unwrap());
        assert_eq!(
            &[10, 11, 2, 3, 4, 5, 6, 7, 8, 9],
            reader.buffer.inner.try_lock().unwrap().as_slice()
        );
        println!("{:?}", writer.buffer);

        let mut buf = [0u8; 8];
        assert_eq!(8, reader.read(buf.as_mut()).unwrap());
        assert_eq!(
            &[3, 4, 5, 6, 7, 8, 9, 10],
            &buf
        );
        assert_eq!(1, reader.buffer.current_reader_location);
        println!("{:?}", writer.buffer);

        let mut buf = [0u8; 3];
        assert_eq!(1, reader.read(buf.as_mut()).unwrap());
        assert_eq!(
            &[11, 0, 0],
            &buf
        );
        assert_eq!(2, reader.buffer.current_reader_location);
        println!("{:?}", writer.buffer);

        let mut buf = [0u8; 20];
        assert_eq!(0, reader.read(buf.as_mut()).unwrap());
        assert_eq!(
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &buf
        );
        assert_eq!(2, reader.buffer.current_reader_location);
        println!("{:?}", writer.buffer);

        assert_eq!(9, writer.write(&[12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31]).unwrap());
        assert_eq!(
            &[20, 11, 12, 13, 14, 15, 16, 17, 18, 19],
            reader.buffer.inner.try_lock().unwrap().as_slice()
        );
        println!("{:?}", writer.buffer);

        assert_eq!(0, writer.write(&[22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41]).unwrap());
        assert_eq!(
            &[20, 11, 12, 13, 14, 15, 16, 17, 18, 19],
            reader.buffer.inner.try_lock().unwrap().as_slice()
        );
        println!("{:?}", writer.buffer);

        let mut buf = [0u8; 20];
        assert_eq!(9, reader.read(buf.as_mut()).unwrap());
        assert_eq!(
            &[12, 13, 14, 15, 16, 17, 18, 19, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &buf
        );
        assert_eq!(1, reader.buffer.current_reader_location);
        println!("{:?}", writer.buffer);

        let mut buf = [0u8; 20];
        assert_eq!(0, reader.read(buf.as_mut()).unwrap());
        assert_eq!(
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &buf
        );
        assert_eq!(1, reader.buffer.current_reader_location);
        println!("{:?}", writer.buffer);
    }
}
