//! Core functionality for Roze standard library
//!
//! This module provides basic types and functions

pub mod types {
    pub struct String {
        data: Vec<u8>,
    }

    impl String {
        pub fn new() -> Self {
            Self { data: Vec::new() }
        }

        pub fn from_str(s: &str) -> Self {
            Self {
                data: s.as_bytes().to_vec(),
            }
        }

        pub fn as_str(&self) -> &str {
            unsafe { std::str::from_utf8_unchecked(&self.data) }
        }
    }

    impl std::fmt::Display for String {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.as_str())
        }
    }
}

pub mod prelude {
    pub use super::types::String;
}

pub fn init() {
    println!("📚 Core library initialized");
}