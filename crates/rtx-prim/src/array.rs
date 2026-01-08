use core::ops::{Deref, DerefMut};

#[repr(C)]
pub struct Array<T, const N: usize> {
    data: [T; N],
    len: usize,
}

impl<T: Default, const N: usize> Array<T, N> {
    pub fn new() -> Self
    where
        T: Default,
    {
        Self {
            data: core::array::from_fn(|_| Default::default()),
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) -> bool {
        if self.len >= N {
            return false;
        }

        self.data[self.len] = value;
        self.len += 1;

        true
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T: Default, const N: usize> Default for Array<T, N>
where
    T: Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Deref for Array<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.data[..self.len]
    }
}

impl<T, const N: usize> DerefMut for Array<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_array_is_empty() {
        let arr: Array<i32, 8> = Array::new();
        assert!(arr.is_empty());
        assert_eq!(arr.len(), 0);
    }

    #[test]
    fn push_increases_len() {
        let mut arr: Array<i32, 8> = Array::new();
        arr.push(1);
        assert_eq!(arr.len(), 1);
        arr.push(2);
        assert_eq!(arr.len(), 2);
        arr.push(3);
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn deref_returns_slice_of_active_elements() {
        let mut arr: Array<i32, 8> = Array::new();
        arr.push(10);
        arr.push(20);
        arr.push(30);

        let slice: &[i32] = &*arr;
        assert_eq!(slice, &[10, 20, 30]);
    }

    #[test]
    fn deref_mut_allows_modification() {
        let mut arr: Array<i32, 8> = Array::new();
        arr.push(1);
        arr.push(2);

        arr[0] = 100;
        assert_eq!(&*arr, &[100, 2]);
    }

    #[test]
    fn default_is_empty() {
        let arr: Array<i32, 4> = Array::default();
        assert!(arr.is_empty());
    }

    #[test]
    fn push_returns_false_when_full() {
        let mut arr: Array<i32, 2> = Array::new();
        assert!(arr.push(1));
        assert!(arr.push(2));
        assert!(!arr.push(3));
        assert_eq!(arr.len(), 2);
        assert_eq!(&*arr, &[1, 2]);
    }
}
