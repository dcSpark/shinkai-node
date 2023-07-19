use std::error::Error;
use std::fmt;

#[derive(Debug)]
/// `InvalidChunkIdError` is an error that occurs when an invalid chunk id is
/// provided to a function.
pub struct InvalidChunkIdError;

impl fmt::Display for InvalidChunkIdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid chunk id")
    }
}

impl Error for InvalidChunkIdError {}

/// `ResourceEmptyError` is an error that occurs when an attempt is made to
/// remove a data chunk and associated embedding from an empty resource.
#[derive(Debug)]
pub struct ResourceEmptyError;

impl fmt::Display for ResourceEmptyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Resource is empty")
    }
}

impl Error for ResourceEmptyError {}
