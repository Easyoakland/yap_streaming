use super::StreamTokensBuffer;
use crate::StreamTokens;
use alloc::string::String;
use yap::Tokens;

/// [`StrStreamTokens`] is like [`StreamTokens`] but optimized for more efficient usage of [`Tokens::parse()`] and related methods when wrapping `Iterator<Item = char>`.
///
/// See [`Self::new`] for example usage.
#[derive(Debug)]
pub struct StrStreamTokens<
    I: Iterator,
    Buffer: StreamTokensBuffer<I::Item> + core::ops::Deref<Target = str>,
>(StreamTokens<I, Buffer>);

impl StreamTokensBuffer<char> for String {
    fn drain_front(&mut self, n: usize) {
        if n > self.len() {
            self.clear()
        } else {
            self.drain(..n).for_each(drop);
        }
    }

    fn push(&mut self, item: char) {
        self.push(item)
    }

    fn get(&self, idx: usize) -> Option<char> {
        self.chars().nth(idx)
    }
}

impl<I> StrStreamTokens<I, String>
where
    I: Iterator<Item = char>,
    I::Item: Clone,
{
    /// Use this method to convert a suitable iterator into [`Tokens`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use yap_streaming::{Tokens, StrStreamTokens};
    ///
    /// // In normal usage, "hello \n\t world".into_tokens()
    /// // would be preferred here (which would give StrTokens).
    /// // This is just to demonstrate using StrStreamTokens:
    /// let chars_iter = "hello \n\t world123".chars();
    /// let mut tokens = StrStreamTokens::new(chars_iter);
    ///
    /// // now we have tokens, we can do some parsing:
    /// assert!(tokens.tokens("hello".chars()));
    /// tokens.skip_while(|c| c.is_whitespace());
    /// assert!(tokens.tokens("world".chars()));
    ///
    /// // And parsing can be efficiently achieved:
    /// assert_eq!(tokens.parse::<u8, String>(), Ok(123));
    /// ```
    pub fn new(iter: I) -> Self {
        Self(StreamTokens::_new(iter))
    }
}

impl<I, Buffer> Tokens for StrStreamTokens<I, Buffer>
where
    I: Iterator,
    I::Item: Clone,
    Buffer: StreamTokensBuffer<I::Item> + core::ops::Deref<Target = str>,
{
    type Item = <StreamTokens<I, Buffer> as Tokens>::Item;

    type Location = <StreamTokens<I, Buffer> as Tokens>::Location;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn location(&self) -> Self::Location {
        self.0.location()
    }

    fn set_location(&mut self, location: Self::Location) {
        self.0.set_location(location)
    }

    fn is_at_location(&self, location: &Self::Location) -> bool {
        self.0.is_at_location(location)
    }

    fn parse<Out, Buf>(&mut self) -> Result<Out, <Out as core::str::FromStr>::Err>
    where
        Out: core::str::FromStr,
        Buf: FromIterator<Self::Item> + core::ops::Deref<Target = str>,
    {
        // Fill rest of buffer with the wrapped stream before parsing everything.
        let from = self.location();
        while self.0.next().is_some() {}
        // Parse everything.
        let res = self.0.buffer.elements[from.cursor - self.0.buffer.oldest_elem_cursor..].parse();
        // Reset location on error.
        if res.is_err() {
            self.set_location(from)
        };
        res
    }
    fn parse_slice<Out, Buf>(
        &mut self,
        from: Self::Location,
        to: Self::Location,
    ) -> Result<Out, <Out as core::str::FromStr>::Err>
    where
        Out: core::str::FromStr,
        Buf: FromIterator<Self::Item> + core::ops::Deref<Target = str>,
    {
        self.0.buffer.elements[from.cursor - self.0.buffer.oldest_elem_cursor
            ..to.cursor - self.0.buffer.oldest_elem_cursor]
            .parse()
    }
    fn parse_take<Out, Buf>(&mut self, n: usize) -> Result<Out, <Out as core::str::FromStr>::Err>
    where
        Out: core::str::FromStr,
        Buf: FromIterator<Self::Item> + core::ops::Deref<Target = str>,
    {
        // Consume the n tokens.
        let from = self.location();
        self.take(n).consume();

        let res = self.0.buffer.elements[from.cursor - self.0.buffer.oldest_elem_cursor
            ..self.0.cursor - self.0.buffer.oldest_elem_cursor]
            .parse();

        // Reset location on error.
        if res.is_err() {
            self.set_location(from);
        }
        res
    }
    fn parse_take_while<Out, Buf, F>(
        &mut self,
        take_while: F,
    ) -> Result<Out, <Out as core::str::FromStr>::Err>
    where
        Out: core::str::FromStr,
        Buf: FromIterator<Self::Item> + core::ops::Deref<Target = str>,
        F: FnMut(&Self::Item) -> bool,
    {
        // Consume all of the tokens matching the function.
        let from = self.location();
        self.take_while(take_while).consume();

        let res = self.0.buffer.elements[from.cursor - self.0.buffer.oldest_elem_cursor
            ..self.0.cursor - self.0.buffer.oldest_elem_cursor]
            .parse();

        // Reset location on error.
        if res.is_err() {
            self.set_location(from);
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_tokens_parse_optimizations_work() {
        // This buffer will panic if it's used.
        struct BadBuffer;
        impl core::iter::FromIterator<char> for BadBuffer {
            fn from_iter<T: IntoIterator<Item = char>>(_: T) -> Self {
                panic!("FromIterator impl shouldn't be used")
            }
        }
        impl core::ops::Deref for BadBuffer {
            type Target = str;
            fn deref(&self) -> &Self::Target {
                panic!("Deref impl shouldn't be used")
            }
        }

        // 0. parse()

        let mut tokens = StrStreamTokens::new("123".chars());

        assert_eq!(tokens.parse::<_, BadBuffer>(), Ok(123));

        // 1. slice(..).parse()

        let mut tokens = StrStreamTokens::new("123abc".chars());

        // Find locations to the number:
        let from = tokens.location();
        tokens.take_while(|t| t.is_numeric()).consume();
        let to = tokens.location();

        let n = tokens
            .slice(from, to)
            .parse::<u16, BadBuffer>()
            .expect("parse worked (1)");

        assert_eq!(n, 123);
        assert_eq!(tokens.collect::<String>(), "abc");

        // 2. take(..).parse()

        let mut tokens = StrStreamTokens::new("123abc".chars());

        let n = tokens
            .take(3)
            .parse::<u16, BadBuffer>()
            .expect("parse worked (2)");

        assert_eq!(n, 123);
        assert_eq!(tokens.collect::<String>(), "abc");

        // 3. take_while(..).parse()

        let mut tokens = StrStreamTokens::new("123abc".chars());

        let n = tokens
            .take_while(|t| t.is_numeric())
            .parse::<u16, BadBuffer>()
            .expect("parse worked (3)");

        assert_eq!(n, 123);
        assert_eq!(tokens.collect::<String>(), "abc");

        // 4. take(..).take_while(..).take(..).parse()

        let mut tokens = StrStreamTokens::new("123ab+=".chars());

        let n = tokens
            .take(6)
            .take(5)
            .take_while(|t| t.is_alphanumeric())
            .take_while(|t| t.is_numeric())
            .take(2)
            .parse::<u16, BadBuffer>()
            .expect("parse worked (4)");

        assert_eq!(n, 12);
        assert_eq!(tokens.collect::<String>(), "3ab+=");
    }
}
