pub struct Array<T, const N: usize> {
    buf: [T; N],
    len: usize,
}

impl<T, const N: usize> Array<T, N>
where
    T: Copy,
{
    pub fn new(default: T) -> Self {
        Self {
            buf: [default; N],
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        match N {
            0 => {}
            1 => {
                self.buf[0] = value;
                self.len = 1;
            }
            _ => {
                if self.len < N {
                    self.buf[self.len] = value;
                    self.len += 1;
                } else {
                    self.buf.copy_within(1..N, 0);
                    self.buf[N - 1] = value;
                }
            }
        }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.buf[..self.len]
    }
}

impl<T, const N: usize> Array<T, N>
where
    T: Default + Copy,
{
    pub fn first(&self) -> T {
        match N {
            0 => T::default(),
            _ => {
                if self.len > 0 {
                    self.buf[0]
                } else {
                    T::default()
                }
            }
        }
    }

    pub fn last(&self) -> T {
        match N {
            0 => T::default(),
            _ => {
                if self.len > 0 {
                    self.buf[self.len - 1]
                } else {
                    T::default()
                }
            }
        }
    }
}

impl<T, const N: usize> Default for Array<T, N>
where
    T: Default + Copy,
{
    fn default() -> Self {
        Self {
            buf: [Default::default(); N],
            len: Default::default(),
        }
    }
}
