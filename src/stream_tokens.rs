use alloc::{collections::VecDeque, rc::Rc, vec::Vec};
use core::{cell::RefCell, fmt::Debug, iter::Iterator};
use yap::{IntoTokens, TokenLocation, Tokens};

/// Buffer over items of an iterator.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Buffer<Item> {
    oldest_elem_id: usize,
    elements: VecDeque<Option<Item>>,
    /// Sorted list of the oldest items needed per location
    checkout: Vec<usize>,
}

// Manual impl because Item: !Default also works.
impl<Item> Default for Buffer<Item> {
    fn default() -> Self {
        Self {
            oldest_elem_id: Default::default(),
            elements: Default::default(),
            checkout: Default::default(),
        }
    }
}

/// Enables parsing a stream of values from an iterator that can't itself be cloned.
/// In order to be able to rewind the iterator it must save values since the oldest not [`Drop`]ed [`StreamTokensLocation`]
#[derive(Debug)]
pub struct StreamTokens<I>
where
    I: Iterator,
{
    iter: I,
    cursor: usize,
    /// Shared buffer of items and the id of the oldest item in the buffer.
    buffer: Rc<RefCell<Buffer<I::Item>>>,
}

/// This implements [`TokenLocation`] and stores the location. It also marks the [`Iterator::Item`]s
/// since it was created to be stored for when the corresponding [`StreamTokens`] is reset.
///
/// The location is equivalent to `offset` in [`Iterator::nth(offset)`].
///
/// The [`Drop`] implementation will un-mark that [`Iterator::Item`]s must be stored,
/// allowing the originating [`StreamTokens`] to drop old values and free memory.
#[derive(Debug)]
pub struct StreamTokensLocation<Item> {
    cursor: usize,
    buffer: Rc<RefCell<Buffer<Item>>>,
}

impl<Item> Clone for StreamTokensLocation<Item> {
    fn clone(&self) -> Self {
        // Checkout the cursor's position again
        let mut buf = self.buffer.borrow_mut();
        let idx = match buf.checkout.binary_search(&self.cursor) {
            Ok(x) | Err(x) => x,
        };
        buf.checkout.insert(idx, self.cursor);
        // Then copy
        Self {
            cursor: self.cursor,
            buffer: self.buffer.clone(),
        }
    }
}

impl<Item> PartialEq for StreamTokensLocation<Item> {
    fn eq(&self, other: &Self) -> bool {
        self.cursor == other.cursor
    }
}
impl<Item> Eq for StreamTokensLocation<Item> {}

impl<Item> Drop for StreamTokensLocation<Item> {
    fn drop(&mut self) {
        let mut buf = self.buffer.borrow_mut();
        // Remove self.cursor from checkout.
        let idx = buf
            .checkout
            .binary_search(&self.cursor)
            .expect("missing entry for location in checkout");
        buf.checkout.remove(idx);
    }
}

impl<Item> TokenLocation for StreamTokensLocation<Item> {
    fn offset(&self) -> usize {
        self.cursor
    }
}

impl<I: Iterator> StreamTokens<I>
where
    I::Item: Clone,
{
    /// We can't define a blanket impl for [`IntoTokens`] on all `impl Iterator<Item: Clone>` without
    /// [specialization](https://rust-lang.github.io/rfcs/1210-impl-specialization.html).
    ///
    /// Instead, use this method to convert a suitable iterator into [`Tokens`].
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
    /// let mut tokens = StreamTokens::into_tokens(chars_iter);
    ///
    /// // now we have tokens, we can do some parsing:
    /// assert!(tokens.tokens("hello".chars()));
    /// tokens.skip_tokens_while(|c| c.is_whitespace());
    /// assert!(tokens.tokens("world".chars()));
    /// ```
    pub fn into_tokens(iter: I) -> Self {
        StreamTokens {
            iter,
            cursor: Default::default(),
            buffer: Default::default(),
        }
    }
}

impl<I> Tokens for StreamTokens<I>
where
    I: Iterator,
    I::Item: Clone + Debug,
{
    type Item = I::Item;

    type Location = StreamTokensLocation<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        self.cursor += 1;

        let mut buf = self.buffer.borrow_mut();

        // Try buffer
        {
            // If buffer has needed element use buffer before getting new elements.
            if let Some(val) = buf
                .elements
                .get(self.cursor - 1 - buf.oldest_elem_id)
                .cloned()
            {
                return val;
            }
        }

        // Clear buffer of old values
        {
            // Remove old values no longer needed by any location
            let min = match buf.checkout.first() {
                Some(&x) => x.min(self.cursor),
                None => self.cursor,
            };
            while (buf.oldest_elem_id < min) && (!buf.elements.is_empty()) {
                buf.elements.pop_front();
                buf.oldest_elem_id += 1;
            }
        }

        // Handle cache miss
        {
            let next = self.iter.next();
            // Don't save to buffer if no locations exist which might need the value again
            if buf.checkout.is_empty() {
                next
            } else {
                buf.elements.push_back(next.clone());
                next
            }
        }
    }

    fn location(&self) -> Self::Location {
        // Checkout value at current location
        let mut buf = self.buffer.borrow_mut();
        match buf.checkout.binary_search(&self.cursor) {
            Ok(x) | Err(x) => buf.checkout.insert(x, self.cursor),
        };
        StreamTokensLocation {
            cursor: self.cursor,
            buffer: Rc::clone(&self.buffer),
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

impl<I> IntoTokens<I::Item> for StreamTokens<I>
where
    I: Iterator,
    I::Item: Clone + core::fmt::Debug,
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
    #[cfg(feature = "alloc")]
    fn stream_tokens_sanity_check() {
        // In reality, one should always prefer to use StrTokens for strings:
        let chars: &mut dyn Iterator<Item = char> = &mut "hello \n\t world".chars();
        // Can't `chars.clone()` so:
        let mut tokens = StreamTokens::into_tokens(chars);

        let loc = tokens.location();
        assert!(tokens.tokens("hello".chars()));

        tokens.set_location(loc.clone());
        assert!(tokens.tokens("hello".chars()));

        tokens.skip_tokens_while(|c| c.is_whitespace());

        assert!(tokens.tokens("world".chars()));

        tokens.set_location(loc);
        assert!(tokens.tokens("hello".chars()));
    }
}
