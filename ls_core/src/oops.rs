//! Error infrastructure for the ls_core crate.
//!
//! Used throughout this crate to represent failure modes visible
//! outside the crate. (Internally too!)

use crate::document;

/// Represents a structured failure type.
/// Typical usage is to return `Result<T, Oops>`.
#[derive(Debug)]
pub enum Oops {
    Ouch(&'static str),
    InvalidIndex(usize, &'static str),
    InvalidPosition(document::Position, &'static str)
}