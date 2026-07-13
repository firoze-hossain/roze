//! Collections library for Roze
//!
//! This module provides data structures like ArrayList, HashMap, etc.

pub mod list {
    pub struct ArrayList<T> {
        data: Vec<T>,
    }

    impl<T> ArrayList<T> {
        pub fn new() -> Self {
            Self { data: Vec::new() }
        }

        pub fn add(&mut self, item: T) {
            self.data.push(item);
        }

        pub fn get(&self, index: usize) -> Option<&T> {
            self.data.get(index)
        }

        pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
            self.data.get_mut(index)
        }

        pub fn remove(&mut self, index: usize) -> Option<T> {
            if index < self.data.len() {
                Some(self.data.remove(index))
            } else {
                None
            }
        }

        pub fn len(&self) -> usize {
            self.data.len()
        }

        pub fn is_empty(&self) -> bool {
            self.data.is_empty()
        }
    }

    impl<T> Default for ArrayList<T> {
        fn default() -> Self {
            Self::new()
        }
    }
}

pub mod map {
    use std::collections::HashMap;

    pub type Map<K, V> = HashMap<K, V>;

    pub fn new<K, V>() -> Map<K, V> {
        HashMap::new()
    }
}