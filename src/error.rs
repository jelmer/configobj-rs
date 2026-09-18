//! Errors produced while parsing or writing config files.

use std::fmt;

/// The kind of problem found in a config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// A line was neither a section header nor a `key = value` pair.
    Parse,
    /// A section header's nesting depth could not be made sense of.
    Nesting,
    /// A section or key was defined twice within the same parent.
    Duplicate,
    /// A value could not be represented in the config file format.
    Unwritable,
}

/// A single problem found at a specific line of a config file.
///
/// `line_number` is 1-based, matching the numbering people see in an editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// What sort of problem this is.
    pub kind: ErrorKind,
    /// Human-readable description, without the line number.
    pub message: String,
    /// 1-based line number the problem was found on.
    pub line_number: usize,
    /// The offending line, as it appeared in the input.
    pub line: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at line {}", self.message, self.line_number)
    }
}

impl std::error::Error for ParseError {}

/// The error type for reading and writing config files.
#[derive(Debug)]
pub enum Error {
    /// One or more lines could not be parsed.
    ///
    /// Parsing continues past a bad line, so this may carry several errors.
    /// The first one is the most useful in a message to the user.
    Parse(Vec<ParseError>),
    /// A value could not be safely represented in the file format.
    Unwritable(String),
    /// The underlying file could not be read or written.
    Io(std::io::Error),
    /// The input was not valid text in the expected encoding.
    Encoding(String),
}

impl Error {
    /// The individual line errors, if this is a parse failure.
    pub fn parse_errors(&self) -> &[ParseError] {
        match self {
            Error::Parse(errors) => errors,
            _ => &[],
        }
    }

    /// The 1-based line number of the first problem, if there is one.
    pub fn line_number(&self) -> Option<usize> {
        self.parse_errors().first().map(|e| e.line_number)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(errors) => match errors.split_first() {
                Some((first, [])) => write!(f, "{first}"),
                Some((first, rest)) => {
                    write!(f, "{first} (and {} more error(s))", rest.len())
                }
                None => f.write_str("parse error"),
            },
            Error::Unwritable(value) => write!(f, "value {value:?} cannot be safely quoted"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Encoding(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Parse(errors) => errors.first().map(|e| e as &dyn std::error::Error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Self {
        Error::Parse(vec![e])
    }
}

pub(crate) fn parse_error(
    kind: ErrorKind,
    message: impl Into<String>,
    line_number: usize,
    line: &str,
) -> ParseError {
    ParseError {
        kind,
        message: message.into(),
        line_number,
        line: line.to_string(),
    }
}

/// A convenient result type for this crate.
pub type Result<T> = std::result::Result<T, Error>;
