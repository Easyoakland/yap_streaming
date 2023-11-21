/*!
This crate builds on the interfaces from [`yap`](https://crates.io/crates/yap) to allow simple parsing of streams.

# Why

There already exist [many](https://github.com/rosetta-rs/parse-rosetta-rs) crates that intend to help with parsing.
Of that list `nom`, `winnow`, `chumsky`, `combine` support parsing streams of values.

`nom`:
- No obvious way to signal the end of a stream to a parser.
- The user of the library has to implement a streaming parser noticeably differently from a non-streaming parser.
- Parsing occurs on chunks. Parsing dynamically sized chunks can require re-parsing the chunk from scratch and redoing work.

`winnow`:
- Parsing occurs on chunks. Parsing dynamically sized chunks can require re-parsing the chunk from scratch and redoing work.

`chumsky` is not designed for speed.

`combine` is complicated.

This crate allows using an already written [`yap`](https://crates.io/crates/yap) parser by simply changing the initial tokens declaration.

```rust
# #[cfg(feature = "alloc")] {
use std::{
    fs::File,
    io::{self, BufReader, Read},
};
use yap_streaming::{
    // Allows you to use `.into_tokens()` on strings and slices,
    // to get an instance of the above:
    IntoTokens,
    // Allows you to get an instance of `Tokens` that supports streams:
    StrStreamTokens,
    // This trait has all of the parsing methods on it:
    Tokens,
};

// Write parser
// =========================================

#[derive(PartialEq, Debug)]
enum Op {
    Plus,
    Minus,
    Multiply,
}
#[derive(PartialEq, Debug)]
enum OpOrDigit {
    Op(Op),
    Digit(u32),
}

// The `Tokens` trait builds on `Iterator`, so we get a `next` method.
fn parse_op(t: &mut impl Tokens<Item = char>) -> Option<Op> {
    let loc = t.location();
    match t.next()? {
        '-' => Some(Op::Minus),
        '+' => Some(Op::Plus),
        'x' => Some(Op::Multiply),
        _ => {
            t.set_location(loc);
            None
        }
    }
}

// We also get other useful functions..
fn parse_digits(t: &mut impl Tokens<Item = char>) -> Option<u32> {
    t.take_while(|c| c.is_digit(10)).parse::<u32, String>().ok()
}

fn parse_all(t: &mut impl Tokens<Item = char>) -> impl Tokens<Item = OpOrDigit> + '_ {
    // As well as combinator functions like `sep_by_all` and `surrounded_by`..
    t.sep_by_all(
        |t| {
            t.surrounded_by(
                |t| parse_digits(t).map(OpOrDigit::Digit),
                |t| {
                    t.skip_while(|c| c.is_ascii_whitespace());
                },
            )
        },
        |t| parse_op(t).map(OpOrDigit::Op),
    )
}

// Now we've parsed our input into OpOrDigits, let's calculate the result..
fn eval(t: &mut impl Tokens<Item = char>) -> u32 {
    let op_or_digit = parse_all(t);
    let mut current_op = Op::Plus;
    let mut current_digit = 0;
    for d in op_or_digit.into_iter() {
        match d {
            OpOrDigit::Op(op) => current_op = op,
            OpOrDigit::Digit(n) => match current_op {
                Op::Plus => current_digit += n,
                Op::Minus => current_digit -= n,
                Op::Multiply => current_digit *= n,
            },
        }
    }
    current_digit
}

// Use parser
// =========================================

// Get our input and convert into something implementing `Tokens`
let mut tokens = "10 + 2 x 12-4,foobar".into_tokens();
// Parse
assert_eq!(eval(&mut tokens), 140);

// Instead of parsing an in-memory buffer we can use `yap_streaming` to parse a stream.
// While we could [`std::io::Read::read_to_end()`] here, what if the file was too large
// to fit in memory? What if we were parsing from a network socket?
let mut io_err = None;
let file_chars = BufReader::new(File::open("examples/opOrDigit.txt").expect("open file"))
    .bytes()
    .map_while(|x| {
        match x {
            Ok(x) => {
                if x.is_ascii() {
                    Some(x as char)
                } else {
                    io_err = Some(io::ErrorKind::InvalidData.into());
                    // Don't parse any further if non-ascii input.
                    // This simple example parser only makes sense with ascii values.
                    None
                }
            }
            Err(e) => {
                io_err = Some(e);
                // Don't parse any further if io error.
                // Alternatively could panic, retry the byte,
                // or include as an error variant and parse Result<char, ParseError> instead.
                None
            }
        }
    });
// Convert to something implementing `Tokens`.
// If parsing a stream not of `char` use [`yap_streaming::StreamTokens`] instead.
let mut tokens = StrStreamTokens::new(file_chars);
// Parse
assert_eq!(eval(&mut tokens), 140);
// Check that parse encountered no io errors.
assert!(io_err.is_none());
# }
```
*/
#![deny(
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs
)]
#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
mod stream_tokens;
#[cfg(feature = "alloc")]
pub use stream_tokens::{str_stream_tokens::StrStreamTokens, StreamTokens, StreamTokensLocation};
pub use yap::{IntoTokens, TokenLocation, Tokens};
