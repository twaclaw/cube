//! Lehmer codes: a bijection between the permutations of `n` things and
//! `0..n!`, used here to number corner and edge permutations.
//!
//! The code of a permutation counts, for each place, how many smaller values
//! come after it. Reading those digits in the factorial number system gives
//! the index, `index = sum(code[i] * (n - 1 - i)!)`.
//!
//! Both directions are `O(n)` and touch no memory but the permutation itself:
//!
//! * the usual `O(n^2)` inversion count is replaced by a bitmask of the values
//!   already placed, so each digit costs one `popcount`;
//! * the table of factorials is replaced by Horner's method, `index = index *
//!   (n - i) + digit`, going the other way with the remainders of `index` by
//!   `1, 2, ... n`;
//! * picking the digit-th value still free is a `trailing_zeros` after
//!   clearing that many low bits, which beats scanning the free values.

/// Permutations of at most this length fit a `u16` mask and a `u32` index.
pub const MAX_LEN: usize = 12;

/// `n!`, for callers that need the size of the domain. Not used in encoding.
pub const fn factorial(n: usize) -> u32 {
    let mut out = 1u32;
    let mut k = 2u32;
    while k <= n as u32 {
        out *= k;
        k += 1;
    }
    out
}

/// The index of `perm`, a permutation of `0..perm.len()`.
pub fn encode(perm: &[u8]) -> u32 {
    debug_assert!(perm.len() <= MAX_LEN);
    let n = perm.len();
    let mut placed: u16 = 0;
    let mut index = 0u32;
    for (i, &value) in perm.iter().enumerate() {
        placed |= 1 << value;
        // Values below this one already placed are not inversions.
        let below = (placed & ((1 << value) - 1)).count_ones();
        index = index * (n - i) as u32 + (u32::from(value) - below);
    }
    index
}

/// The permutation `index` stands for, written into `perm`.
pub fn decode(index: u32, perm: &mut [u8]) {
    debug_assert!(perm.len() <= MAX_LEN);
    let n = perm.len();
    // The digits come out last one first, with radix 1, 2, ... n.
    let mut digits = [0u8; MAX_LEN];
    let mut rest = index;
    for i in (0..n).rev() {
        let radix = (n - i) as u32;
        digits[i] = (rest % radix) as u8;
        rest /= radix;
    }
    let mut free: u16 = (1u16 << n) - 1;
    for (slot, &digit) in perm.iter_mut().zip(&digits[..n]) {
        let mut bits = free;
        for _ in 0..digit {
            bits &= bits - 1; // drop the lowest value still free
        }
        let value = bits.trailing_zeros() as u8;
        free &= !(1 << value);
        *slot = value;
    }
}
