use alloc::{collections::VecDeque, rc::Rc, vec::Vec};
use core::{
    cell::RefCell,
    fmt::Debug,
    iter::{Fuse, Iterator},
};
use yap::{IntoTokens, TokenLocation, Tokens};

pub(crate) mod str_stream_tokens;

/// Helper trait for defining buffers that can be used to store items in [`StreamTokens`] for [`Tokens::set_location()`] resets
pub trait StreamTokensBuffer<Item>: Default {
    /// Remove n items from the front of the buffer. If buffer has less than `n` elements clear the buffer.
    fn drain_front(&mut self, n: usize);
    /// Add a new item to the back of the buffer.
    fn push(&mut self, item: Item);
    /// Get the item at the given `idx` if it exists.
    fn get(&self, idx: usize) -> Option<Item>;
}

impl<Item: core::clone::Clone> StreamTokensBuffer<Item> for VecDeque<Item> {
    fn drain_front(&mut self, n: usize) {
        if n >= self.len() {
            self.clear()
        } else {
            // TODO test this vs self.drain(..n) performance
            for _ in 0..n {
                self.pop_front();
            }
        }
    }

    fn push(&mut self, item: Item) {
        self.push_back(item)
    }

    fn get(&self, idx: usize) -> Option<Item> {
        self.get(idx).cloned()
    }
}

/// Buffer over items of an iterator.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
struct Buffer<Buf> {
    oldest_elem_cursor: usize,
    elements: Buf,
}

/// Enables parsing a stream of values from a [`Fuse`]d iterator that can't itself be cloned.
/// In order to be able to rewind the iterator it must save values since the oldest not [`Drop`]ed [`StreamTokensLocation`] into `Buf`.
///
/// See [`Self::new`] for example usage.
#[derive(Debug)]
pub struct StreamTokens<I, Buf>
where
    I: Iterator,
{
    iter: Fuse<I>,
    cursor: usize,
    buffer: Buffer<Buf>,
    /// Sorted list of the oldest items needed per live location
    checkout: Rc<RefCell<Vec<usize>>>,
}

/// This implements [`TokenLocation`] and stores the location. It also marks the [`Iterator::Item`]s
/// since it was created to be stored for when the corresponding [`StreamTokens`] is reset.
///
/// The location is equivalent to `offset` in [`Iterator::nth(offset)`].
///
/// The [`Drop`] implementation will un-mark that [`Iterator::Item`]s must be stored,
/// allowing the originating [`StreamTokens`] to drop old values and free memory.
#[derive(Debug)]
pub struct StreamTokensLocation {
    cursor: usize,
    checkout: Rc<RefCell<Vec<usize>>>,
}

impl Clone for StreamTokensLocation {
    fn clone(&self) -> Self {
        // Checkout the cursor's position again
        let mut checkout = self.checkout.borrow_mut();
        let idx = match checkout.binary_search(&self.cursor) {
            Ok(x) | Err(x) => x,
        };
        checkout.insert(idx, self.cursor);
        // Then copy
        Self {
            cursor: self.cursor,
            checkout: Rc::clone(&self.checkout),
        }
    }
}

impl PartialEq for StreamTokensLocation {
    fn eq(&self, other: &Self) -> bool {
        self.cursor == other.cursor
    }
}
impl Eq for StreamTokensLocation {}

impl Drop for StreamTokensLocation {
    fn drop(&mut self) {
        let mut checkout = self.checkout.borrow_mut();
        // Remove self.cursor from checkout.
        let idx = checkout
            .binary_search(&self.cursor)
            .expect("missing entry for location in checkout");
        checkout.remove(idx);
    }
}

impl TokenLocation for StreamTokensLocation {
    fn offset(&self) -> usize {
        self.cursor
    }
}

impl<I: Iterator, Buf: Default> StreamTokens<I, Buf> {
    /// Generic new function allowing arbitrary buffer.
    /// Exists because type inference is not smart enough to try the default generic when calling [`Self::new`] so `new` hardcodes the default.
    /// See <https://faultlore.com/blah/defaults-affect-inference/#default-type-parameters>
    pub(crate) fn _new(iter: I) -> Self {
        StreamTokens {
            // Store a fused iterator so the buffer can safely be of `Item` instead of `Option<Item>`
            iter: iter.fuse(),
            cursor: Default::default(),
            buffer: Default::default(),
            checkout: Default::default(),
        }
    }
}

impl<I: Iterator> StreamTokens<I, VecDeque<I::Item>>
where
    I::Item: Clone,
{
    /// Use this method to convert a suitable iterator into [`Tokens`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use yap_streaming::{Tokens, StreamTokens};
    ///
    /// // In normal usage, "hello \n\t world".into_tokens()
    /// // would be preferred here (which would give StrTokens).
    /// // This is just to demonstrate using StreamTokens:
    /// let chars_iter = "hello \n\t world".chars();
    /// let mut tokens = StreamTokens::new(chars_iter);
    ///
    /// // now we have tokens, we can do some parsing:
    /// assert!(tokens.tokens("hello".chars()));
    /// tokens.skip_while(|c| c.is_whitespace());
    /// assert!(tokens.tokens("world".chars()));
    /// ```
    pub fn new(iter: I) -> Self {
        Self::_new(iter)
    }
}

impl<I, Buffer> Tokens for StreamTokens<I, Buffer>
where
    I: Iterator,
    I::Item: Clone,
    Buffer: StreamTokensBuffer<I::Item>,
{
    type Item = I::Item;

    type Location = StreamTokensLocation;

    fn next(&mut self) -> Option<Self::Item> {
        self.cursor += 1;

        // Try buffer
        {
            // If buffer has needed element use buffer before getting new elements.
            if let Some(val) = self
                .buffer
                .elements
                .get(self.cursor - 1 - self.buffer.oldest_elem_cursor)
            {
                return Some(val);
            }
        }

        let checkout = self.checkout.borrow();
        // Clear buffer of old values
        {
            // Remove old values no longer needed by any location
            let min = match checkout.first() {
                Some(&x) => x.min(self.cursor),
                None => self.cursor,
            };
            let delta = min - self.buffer.oldest_elem_cursor;
            self.buffer.elements.drain_front(delta);
            self.buffer.oldest_elem_cursor = min;
        }

        // Handle cache miss
        {
            let next = self.iter.next()?;
            // Don't save to buffer if no locations exist which might need the value again
            if checkout.is_empty() {
                Some(next)
            } else {
                self.buffer.elements.push(next.clone());
                Some(next)
            }
        }
    }

    fn location(&self) -> Self::Location {
        // Checkout value at current location
        let mut checkout = self.checkout.borrow_mut();
        match checkout.binary_search(&self.cursor) {
            Ok(x) | Err(x) => checkout.insert(x, self.cursor),
        };
        StreamTokensLocation {
            cursor: self.cursor,
            checkout: Rc::clone(&self.checkout),
        }
    }

    fn set_location(&mut self, location: Self::Location) {
        // Update cursor to new value
        self.cursor = location.cursor;
        // Location removes itself from checkout on drop
    }

    fn is_at_location(&self, location: &Self::Location) -> bool {
        self.cursor == location.cursor
    }
}

impl<I, Buf> IntoTokens<I::Item> for StreamTokens<I, Buf>
where
    I: Iterator,
    I::Item: Clone + core::fmt::Debug,
    Buf: StreamTokensBuffer<I::Item>,
{
    type Tokens = Self;
    fn into_tokens(self) -> Self {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_tokens_sanity_check() {
        // In reality, one should always prefer to use StrTokens for strings:
        let chars: &mut dyn Iterator<Item = char> = &mut "hello \n\t world".chars();
        // Can't `chars.clone()` so:
        let mut tokens = StreamTokens::new(chars);

        let loc = tokens.location();
        assert!(tokens.tokens("hello".chars()));

        tokens.set_location(loc.clone());
        assert!(tokens.tokens("hello".chars()));

        tokens.skip_while(|c| c.is_whitespace());

        assert!(tokens.tokens("world".chars()));

        tokens.set_location(loc);
        assert!(tokens.tokens("hello \n\t world".chars()));

        assert_eq!(None, tokens.next())
    }

    #[test]
    fn str_stream_tokens_sanity_check() {
        // In reality, one should always prefer to use StrTokens for strings:
        let chars: &mut dyn Iterator<Item = char> = &mut "hello \n\t world".chars();
        // Can't `chars.clone()` so:
        let mut tokens = crate::StrStreamTokens::new(chars);

        let loc = tokens.location();
        assert!(tokens.tokens("hello".chars()));

        tokens.set_location(loc.clone());
        assert!(tokens.tokens("hello".chars()));

        tokens.skip_while(|c| c.is_whitespace());

        assert!(tokens.tokens("world".chars()));

        tokens.set_location(loc);
        assert!(tokens.tokens("hello \n\t world".chars()));

        assert_eq!(None, tokens.next())
    }
}
