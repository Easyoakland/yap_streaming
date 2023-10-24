#![doc = include_str!("../README.md")]
//! ```rust
//! # #[cfg(feature = "alloc")] {
#![doc = include_str!("../examples/fizzbuzz.rs")]
//! # }
//! ```
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
mod stream_tokens;
#[cfg(feature = "alloc")]
pub use stream_tokens::{StreamTokens, StreamTokensLocation};
pub use yap::{IntoTokens, TokenLocation, Tokens};
