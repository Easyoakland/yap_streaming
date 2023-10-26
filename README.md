This crate builds on the interfaces from [`yap`](crates.io/crates/yap) to allow simple parsing of streams.

# Why

There already exist [many](https://github.com/rosetta-rs/parse-rosetta-rs) crates that intend to help with parsing.
Of that list `nom`, `winnow`, `chumsky`, `combine` support parsing streams of values.

`nom` and `winnow` are very similar and share a few problems:
- No obvious way to signal the end of a stream to a parser.
- Running out of input requires re-parsing from scratch and redoing work.
- The user of the library has to implement a streaming parser noticeably differently from a non-streaming parser.

`chumsky` is not designed for speed.

`combine` is complicated.

This crate allows using an already written [`yap`](crates.io/crates/yap) parser by simply changing the initial tokens declaration.

```rust
# #[cfg(feature = "alloc")] {
use std::{
    fs::File,
    io::{BufReader, Read},
};
use yap_streaming::{
    // This trait has all of the parsing methods on it:
    Tokens,
    // Allows you to use `.into_tokens()` on strings and slices,
    // to get an instance of the above:
    IntoTokens,
    // Allows you to get an instance of `Tokens` that supports streams:
    StreamTokens,
};

// Write parser
// =========================================

#[derive(PartialEq, Debug)]
enum Op { Plus, Minus, Multiply }
#[derive(PartialEq, Debug)]
enum OpOrDigit { Op(Op), Digit(u32) }

// The `Tokens` trait builds on `Iterator`, so we get a `next` method.
fn parse_op(t: &mut impl Tokens<Item=char>) -> Option<Op> {
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
fn parse_digits(t: &mut impl Tokens<Item=char>) -> Option<u32> {
    let s: String = t
        .tokens_while(|c| c.is_digit(10))
        .collect();
    s.parse().ok()
}

fn parse_all(t: &mut impl Tokens<Item=char>) -> impl Iterator<Item = OpOrDigit> + '_ {
    // As well as combinator functions like `sep_by_all` and `surrounded_by`..
    t.sep_by_all(
        |t| t.surrounded_by(
            |t| parse_digits(t).map(OpOrDigit::Digit),
            |t| { t.skip_tokens_while(|c| c.is_ascii_whitespace()); }
        ),
        |t| parse_op(t).map(OpOrDigit::Op)
    )
}

// Now we've parsed our input into OpOrDigits, let's calculate the result..
fn eval(t: &mut impl Tokens<Item = char>) -> u32 {
    let op_or_digit = parse_all(t);
    let mut current_op = Op::Plus;
    let mut current_digit = 0;
    for d in op_or_digit {
        match d {
            OpOrDigit::Op(op) => {
                current_op = op
            },
            OpOrDigit::Digit(n) => {
                match current_op {
                    Op::Plus => { current_digit += n },
                    Op::Minus => { current_digit -= n },
                    Op::Multiply => { current_digit *= n },
                }
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
// While we could just [`std::io::Read::read_to_end()`] here, what if the file was too large to fit in memory?
// What if we were parsing from a network socket?
let file_bytes = BufReader::new(File::open("examples/opOrDigit.txt").unwrap())
    .bytes()
    .map_while(|x| x.ok().map(|x| x as char));
// Convert to something implementing `Tokens`
let mut tokens = StreamTokens::into_tokens(file_bytes);
// Parse
assert_eq!(eval(&mut tokens), 140);
# }
```
