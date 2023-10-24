/*!
This crate builds on the interfaces from [`yap`] to allow simple parsing of streams.

# Why

There already exist [many](https://github.com/rosetta-rs/parse-rosetta-rs) crates that intend to help with parsing.
Of that list `nom`, `winnow`, `chumsky`, `combine` support parsing streams of values.

`nom` and `winnow` are very similar and share a few problems:
- No obvious way to signal the end of a stream to a parser.
- Running out of input requires re-parsing from scratch and redoing work.
- The user of the library has to implement a streaming parser noticeably differently from a non-streaming parser.

`chumsky` is not designed for speed.

`combine` is complicated.

This crate allows using an already written [`yap`] parser by simply changing the initial tokens declaration.

```rust
    # #[cfg(feature = "std")] {
    # use yap_streaming::{IntoTokens, StreamTokens, Tokens};
    fn hello_world(tokens: &mut impl Tokens<Item = char>) -> Option<&'static str> {
        if tokens.tokens("hello".chars()) {
            tokens.skip_tokens_while(|c| c.is_whitespace());
            if tokens.tokens("world".chars()) {
                Some("hello world")
            } else {
                Some("hello")
            }
        } else {
            if tokens.tokens("world".chars()) {
                Some("world")
            } else {
                None
            }
        }
    }
    let msg = "hello world";
    assert_eq!(hello_world(&mut msg.into_tokens()), hello_world(&mut StreamTokens::into_tokens(msg.chars())));
    let msg = " world";
    assert_eq!(hello_world(&mut msg.into_tokens()), hello_world(&mut StreamTokens::into_tokens(msg.chars())));
    let msg = "world";
    assert_eq!(hello_world(&mut msg.into_tokens()), hello_world(&mut StreamTokens::into_tokens(msg.chars())));
    let msg = "";
    assert_eq!(hello_world(&mut msg.into_tokens()), hello_world(&mut StreamTokens::into_tokens(msg.chars())));
    # }
```
*/
//! However it can also use that parser on a stream of values that are not all in memory:
//! ```rust
//! # #[cfg(feature = "std")] {
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
