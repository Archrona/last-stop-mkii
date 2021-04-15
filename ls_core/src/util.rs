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
