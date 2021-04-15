//! Error infrastructure for the ls_core crate.
//!
//! Used throughout this crate to represent failure modes visible
//! outside the crate. (Internally too!)

use crate::document;
use std::ops::{Bound, RangeBounds};

/// Represents a structured failure type.
/// Typical usage is to return `Result<T, Oops>`.
#[derive(PartialEq, Eq, Debug)]
pub enum Oops {
    Ouch(&'static str),
    NonexistentAnchor(document::AnchorHandle),
    CannotRemoveAnchor(document::AnchorHandle),
    NoMoreUndos(usize),
    NoMoreRedos(usize),
    InvalidIndex(usize, &'static str),
    InvalidPosition(document::Position, &'static str),
    InvalidRange(document::Range, &'static str),
    EmptyString(&'static str)
}

/// Returns the substring of `s` starting at Unicode codepoint index `start`
/// and extending for `len` codepoints.
/// 
/// Adapted from: carlomilanesi
/// https://users.rust-lang.org/t/how-to-get-a-substring-of-a-string/1351/11
pub fn substring(s: &str, start: usize, len: usize) -> &str {
    let mut char_pos = 0;
    let mut byte_start = 0;
    let mut it = s.chars();
    loop {
        if char_pos == start { break; }
        if let Some(c) = it.next() {
            char_pos += 1;
            byte_start += c.len_utf8();
        }
        else { break; }
    }
    char_pos = 0;
    let mut byte_end = byte_start;
    loop {
        if char_pos == len { break; }
        if let Some(c) = it.next() {
            char_pos += 1;
            byte_end += c.len_utf8();
        }
        else { break; }
    }
    &s[byte_start..byte_end]
}

/// Returns the slice of `s` given by Unicode codepoint indices `range`.
/// 
/// Adapted from: carlomilanesi
/// https://users.rust-lang.org/t/how-to-get-a-substring-of-a-string/1351/11
pub fn slice(s: &str, range: impl RangeBounds<usize>) -> &str {
    let start = match range.start_bound() {
        Bound::Included(bound) | Bound::Excluded(bound) => *bound,
        Bound::Unbounded => 0,
    };
    let len = match range.end_bound() {
        Bound::Included(bound) => *bound + 1,
        Bound::Excluded(bound) => *bound,
        Bound::Unbounded => s.len(),
    } - start;
    substring(s, start, len)
}

/// Returns the utf-8 codepoint index corresponding to byte offset `byte`
/// in string `s`, or `None` if the byte offset is out of range or not a valid
/// UTF-8 character.
/// 
/// If `byte` is equal to the length of `s` in bytes, returns the number
/// of characters in `s`. This is useful for anchor/cursor manipulations.
/// 
/// # Examples
/// ```
/// use ls_core::util::*;
/// let s = "Ɣa🙈◧";  // hex: c6 94, 61, f0 9f 99 88, e2 97 a7
/// assert_eq!(byte_index_to_cp(&s, 0), Some(0));
/// assert_eq!(byte_index_to_cp(&s, 1), None);
/// assert_eq!(byte_index_to_cp(&s, 2), Some(1));
/// assert_eq!(byte_index_to_cp(&s, 3), Some(2));
/// assert_eq!(byte_index_to_cp(&s, 4), None);
/// assert_eq!(byte_index_to_cp(&s, 5), None);
/// assert_eq!(byte_index_to_cp(&s, 6), None);
/// assert_eq!(byte_index_to_cp(&s, 7), Some(3));
/// assert_eq!(byte_index_to_cp(&s, 8), None);
/// assert_eq!(byte_index_to_cp(&s, 9), None);
/// assert_eq!(byte_index_to_cp(&s, 10), Some(4));
/// ```
pub fn byte_index_to_cp(s: &str, byte: usize) -> Option<usize> {
    let mut cp_index = 0;

    for (b, _) in s.char_indices() {
        if b > byte {
            return None;
        } else if b == byte {
            return Some(cp_index);
        } else {
            cp_index += 1;
        }
    }
    
    if byte == s.len() {
        Some(cp_index)
    } else {
        None
    }
}

/// Returns the byte index of the `cp`th unicode codepoint in `s`,
/// or `None` if the supplied index is out of range.
/// 
/// If `cp` is equal to the length of `s` in chars, returns the number
/// of bytes in `s`. This is useful for anchor/cursor manipulations.
/// 
/// # Examples
/// ```
/// use ls_core::util::*;
/// let s = "Ɣa🙈◧";  // hex: c6 94, 61, f0 9f 99 88, e2 97 a7
/// assert_eq!(cp_index_to_byte(&s, 0), Some(0));
/// assert_eq!(cp_index_to_byte(&s, 1), Some(2));
/// assert_eq!(cp_index_to_byte(&s, 2), Some(3));
/// assert_eq!(cp_index_to_byte(&s, 3), Some(7));
/// assert_eq!(cp_index_to_byte(&s, 4), Some(10));
/// assert_eq!(cp_index_to_byte(&s, 5), None);
/// ```
pub fn cp_index_to_byte(s: &str, cp: usize) -> Option<usize> {
    let mut cp_index = 0;

    for (b, _) in s.char_indices() {
        if cp_index > cp {
            return None;
        } else if cp_index == cp {
            return Some(b);
        } else {
            cp_index += 1;
        }
    }
    
    if cp_index == cp {
        Some(s.len())
    } else {
        None
    }
}