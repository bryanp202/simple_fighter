pub struct RingBuf<T: Clone + Default, const SIZE: usize> {
    buf: [T; SIZE],
    index: usize,
}

impl <T: Clone + Default, const SIZE: usize> RingBuf<T, SIZE> {
    pub fn new() -> Self {
        Self {buf: std::array::from_fn(|_| T::default()), index: 0}
    }

    pub fn append(&mut self, new_data: &T) {
        self.index = (self.index + 1) % SIZE;
        self.buf[self.index] = new_data.clone();
    }

    pub fn rewind(&mut self, indices: usize) -> T {
        self.index = (SIZE + self.index - indices) % SIZE;
        self.buf[self.index].clone()
    } 
}
