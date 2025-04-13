pub(crate) const QUEUE_BUFFER_LEN: usize = 256;

pub(crate) struct QueueBuffer {
    buf: [u8; QUEUE_BUFFER_LEN],
    e: usize,
    f: usize,
}

impl QueueBuffer {
    pub fn new() -> Self {
        Self {
            buf: [0; QUEUE_BUFFER_LEN],
            e: 0,
            f: 0,
        }
    }
    pub fn push(&mut self, val: u8) {
        self.buf[self.f] = val;
        self.f = (self.f + 1) % QUEUE_BUFFER_LEN;
    }
    pub fn top(&self) -> u8 {
        if self.e == self.f {
            0xff
        } else {
            self.buf[self.e]
        }
    }

    pub fn pop(&mut self) -> u8 {
        if self.e == self.f {
            0xff
        } else {
            let ret = self.buf[self.e];
            self.e = (self.e + 1) % QUEUE_BUFFER_LEN;
            ret
        }
    }
}
