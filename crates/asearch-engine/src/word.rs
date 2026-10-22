//! Fixed-capacity state words. Bit zero is the start position.

use std::fmt::Debug;

mod sealed {
    pub trait Sealed {}
}

/// State operations used by the scalar Ideal matcher.
///
/// Sealed so capacity and storage accounting cannot be overridden by callers.
pub trait BitWord: sealed::Sealed + Copy + Debug + Eq {
    const BITS: usize;

    fn zero() -> Self;

    fn bit(position: usize) -> Self;

    fn union(self, other: Self) -> Self;

    fn intersection(self, other: Self) -> Self;

    fn shift(self) -> Self;
}

macro_rules! integer_word {
    ($($word:ty),+) => {$(
        impl sealed::Sealed for $word {}

        impl BitWord for $word {
            const BITS: usize = <$word>::BITS as usize;

            fn zero() -> Self { 0 }

            fn bit(position: usize) -> Self {
                if position < <Self as BitWord>::BITS { 1 << position } else { 0 }
            }

            fn union(self, other: Self) -> Self { self | other }

            fn intersection(self, other: Self) -> Self { self & other }

            fn shift(self) -> Self { self << 1 }
        }
    )+};
}

integer_word!(u32, u64, u128);

/// Little-endian words with carry between adjacent words and no heap storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiWord<const N: usize>([u64; N]);

impl<const N: usize> sealed::Sealed for MultiWord<N> {}

impl<const N: usize> BitWord for MultiWord<N> {
    const BITS: usize = N.saturating_mul(64);

    fn zero() -> Self {
        Self([0; N])
    }

    fn bit(position: usize) -> Self {
        let mut value = Self::zero();
        if let Some(word) = value.0.get_mut(position / 64) {
            *word = 1 << (position % 64);
        }
        value
    }

    fn union(mut self, other: Self) -> Self {
        for (word, rhs) in self.0.iter_mut().zip(other.0) {
            *word |= rhs;
        }
        self
    }

    fn intersection(mut self, other: Self) -> Self {
        for (word, rhs) in self.0.iter_mut().zip(other.0) {
            *word &= rhs;
        }
        self
    }

    fn shift(mut self) -> Self {
        let mut carry = 0;
        for word in &mut self.0 {
            let next = *word >> 63;
            *word = (*word << 1) | carry;
            carry = next;
        }
        self
    }
}
