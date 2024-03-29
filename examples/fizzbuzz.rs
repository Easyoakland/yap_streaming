use std::io::{stdin, Read};
use yap_streaming::{StreamTokens, Tokens};

/// Parses a line ending of either "\r\n" (windows) or "\n" (linux)
fn line_ending(tokens: &mut impl Tokens<Item = u8>) -> Option<&'static str> {
    tokens
        .optional(|t| {
            t.tokens("\r\n".chars().map(|x| u8::try_from(x).unwrap()))
                .then_some("\r\n")
        })
        .or_else(|| tokens.optional(|t| t.token(u8::try_from('\n').unwrap()).then_some("\n")))
}

#[derive(Debug)]
enum FizzBuzz {
    Fizz,
    Buzz,
    Both,
    Neither,
}

/// Converts number to appropriate fizzbuzz type.
fn fizzbuzz(x: u32) -> FizzBuzz {
    if x % 5 == 0 && x % 3 == 0 {
        FizzBuzz::Both
    } else if x % 3 == 0 {
        FizzBuzz::Fizz
    } else if x % 5 == 0 {
        FizzBuzz::Buzz
    } else {
        FizzBuzz::Neither
    }
}

/// An example program that parses stdin for numbers that match an output by the rules of fizzbuzz.
/// It prints as it parses so the incremental parsing is more obvious.
fn main() {
    let stdin = stdin().bytes().map(Result::unwrap);
    // Can't use `stdin.clone()` because it is over a stream of values that are not in memory.
    // If we could clone then one of the other yap token types would be preferable.
    let mut tokens = StreamTokens::new(stdin);
    let mut parsed_result = Vec::new();

    println!("Lets play fizzbuzz! Enter a number. If it is divisible by three I'll say \"fizz\", \
if it is divisible by five I'll say \"buzz\", and if it is divisible by both 3 and 5 I'll say \"fizzbuzz\".");

    // Set a location to allow rewinding back to this point later.
    // All items since the oldest location (`start` in this case) will be buffered
    // so that a rewind to this point can occur with [`Tokens::set_location`] if needed.
    let start = tokens.location();

    // Demonstrate streaming parsing of input.
    // This is relatively painless and looks the same as a non-streaming parser.
    // `StreamTokens` handles buffering and `Bytes<Stdin>` handles blocking in this case.
    // The general form is some iterator over new input and wrapping that in `StreamTokens`
    // to make that stream of items parseable.
    for num in tokens
        .sep_by(
            |t| {
                Some(
                    t.take_while(u8::is_ascii_digit)
                        .into_iter()
                        .map(|x| x as char)
                        .collect::<String>()
                        .parse::<u32>(),
                )
            },
            |t| line_ending(t).is_some(),
        )
        .into_iter()
    {
        match num {
            Ok(x) => {
                let res = fizzbuzz(x);
                println!("{res:?}");
                parsed_result.push(res);
            }
            Err(_) => println!("Ending parse on invalid u64"),
        }
    }

    // Can use location to reset to previous values because they have been internally buffered
    let previous_tokens = tokens
        .slice(start, tokens.location())
        .as_iter()
        .map(|x| x as char)
        .collect::<String>();
    println!("You entered:\n{previous_tokens}");
    println!("This parsed as: {parsed_result:?}");
}
