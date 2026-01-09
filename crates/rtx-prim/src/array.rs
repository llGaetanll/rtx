use core::ops::{Index, IndexMut};

#[repr(C)]
pub struct Array<T, const N: usize> {
    data: [T; N],
    len: usize,
}

impl<T: Copy + Default, const N: usize> Array<T, N> {
    pub fn new() -> Self {
        Self {
            data: [T::default(); N],
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

impl<T: Copy + Default, const N: usize> Default for Array<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> From<[T; N]> for Array<T, N> {
    fn from(data: [T; N]) -> Self {
        Self { data, len: N }
    }
}

impl<T, const N: usize> Index<usize> for Array<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl<T, const N: usize> IndexMut<usize> for Array<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
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
    fn index_returns_elements() {
        let mut arr: Array<i32, 8> = Array::new();
        arr.push(10);
        arr.push(20);
        arr.push(30);

        assert_eq!(arr[0], 10);
        assert_eq!(arr[1], 20);
        assert_eq!(arr[2], 30);
    }

    #[test]
    fn index_mut_allows_modification() {
        let mut arr: Array<i32, 8> = Array::new();
        arr.push(1);
        arr.push(2);

        arr[0] = 100;
        assert_eq!(arr[0], 100);
        assert_eq!(arr[1], 2);
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
        assert_eq!(arr[0], 1);
        assert_eq!(arr[1], 2);
    }

    #[test]
    fn from_array_sets_len_to_capacity() {
        let arr: Array<i32, 3> = Array::from([10, 20, 30]);
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], 10);
        assert_eq!(arr[1], 20);
        assert_eq!(arr[2], 30);
    }

    #[test]
    fn from_array_allows_indexing() {
        let arr: Array<i32, 3> = Array::from([5, 10, 15]);
        assert_eq!(arr[0], 5);
        assert_eq!(arr[1], 10);
        assert_eq!(arr[2], 15);
    }
}
