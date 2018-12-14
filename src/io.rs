// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of
// this software and associated documentation files (the "Software"), to deal in
// the Software without restriction, including without limitation the rights to
// use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software is furnished to do so,
// subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
// FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS
// OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
// WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use crate::encode;
use futures::Poll;
use smallvec::SmallVec;
use std::{io, usize};
use tokio_io::AsyncWrite;

#[derive(Debug)]
pub struct UviWriter<T> {
    io: T,
    buf: [u8; encode::USIZE_LEN],
    idx: usize,
    len: usize,
    pfx: SmallVec<[u8; 8]>,
    sfx: SmallVec<[u8; 8]>,
    state: State
}

#[derive(Debug)]
enum State {
    Init,
    WriteLen,
    WritePrefix,
    WriteData,
    WriteSuffix(usize)
}

impl<T> UviWriter<T> {
    pub fn new(io: T) -> Self {
        UviWriter {
            io,
            buf: encode::usize_buffer(),
            idx: 0,
            len: 0,
            pfx: SmallVec::new(),
            sfx: SmallVec::new(),
            state: State::Init
        }
    }

    pub fn into_inner(self) -> T {
        self.io
    }

    /// Set constant value to put before each item to write.
    pub fn set_prefix(&mut self, val: &[u8]) {
        self.pfx.clear();
        self.pfx.extend_from_slice(val);
    }

    /// Set constant value to put after each item to write.
    pub fn set_suffix(&mut self, val: &[u8]) {
        self.sfx.clear();
        self.sfx.extend_from_slice(val);
    }
}

impl<T: io::Write> io::Write for UviWriter<T> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        loop {
            match self.state {
                State::Init => {
                    let len = self.pfx.len() + self.sfx.len() + data.len();
                    let uvi = encode::usize(len, &mut self.buf);
                    self.len = uvi.len();
                    self.idx = self.io.write(uvi)?;
                    if self.idx < self.len {
                        self.state = State::WriteLen
                    } else {
                        self.len = data.len();
                        self.idx = 0;
                        self.state = State::WritePrefix
                    }
                }
                State::WriteLen => {
                    self.idx += self.io.write(&self.buf[self.idx .. self.len])?;
                    if self.idx >= self.len {
                        self.len = data.len();
                        self.idx = 0;
                        self.state = State::WritePrefix
                    }
                }
                State::WritePrefix => {
                    self.idx += self.io.write(&self.pfx[self.idx ..])?;
                    if self.idx >= self.pfx.len() {
                        self.idx = 0;
                        self.state = State::WriteData
                    }
                }
                State::WriteData => {
                    let n = self.io.write(data)?;
                    self.idx += n;
                    if self.idx < self.len {
                        return Ok(n)
                    } else {
                        self.idx = 0;
                        self.state = State::WriteSuffix(n)
                    }
                }
                State::WriteSuffix(n) => {
                    self.idx += self.io.write(&self.sfx[self.idx ..])?;
                    if self.idx >= self.sfx.len() {
                        self.state = State::Init;
                        return Ok(n)
                    }
                }
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

impl<T: AsyncWrite> AsyncWrite for UviWriter<T> {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.io.shutdown()
    }
}
