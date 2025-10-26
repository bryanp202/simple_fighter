use std::mem::MaybeUninit;

pub struct RingBuf<T: Clone, const SIZE: usize> {
    buf: [MaybeUninit<T>; SIZE],
    index: usize,
}

impl<T: Clone, const SIZE: usize> RingBuf<T, SIZE> {
    pub fn new(start_state: T) -> Self {
        Self {
            buf: std::array::from_fn(|_| MaybeUninit::new(start_state.clone())),
            index: 0,
        }
    }

    pub fn append(&mut self, new_data: T) {
        self.index = (self.index + 1) % SIZE;
        self.buf[self.index] = MaybeUninit::new(new_data.clone());
    }

    pub fn rewind(&mut self, indices: usize) -> T {
        self.index = (SIZE + self.index - indices) % SIZE;
        unsafe { self.buf[self.index].assume_init_read() }
    }
}
