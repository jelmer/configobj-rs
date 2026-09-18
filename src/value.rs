//! Scalar values stored in a config file.

use std::fmt;

/// A value attached to a key.
///
/// The file format itself is untyped: everything is text, and the only
/// structural distinction it makes is between a single value and a
/// comma-separated list. Interpreting text as a bool or a number is left to
/// the accessors ([`Value::as_bool`] and friends).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A single value, such as `name = Jelmer`.
    String(String),
    /// A comma-separated list, such as `names = Jelmer, Ada`.
    List(Vec<String>),
}

/// The value could not be read as a boolean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotABool {
    /// The text was not one of the recognised boolean spellings.
    Unrecognised(String),
    /// The value was a list, so there was nothing single to interpret.
    WrongShape(WrongShape),
}

impl fmt::Display for NotABool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotABool::Unrecognised(text) => write!(f, "{text:?} is not a valid boolean"),
            NotABool::WrongShape(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for NotABool {}

/// The value was a list where a single value was wanted, or vice versa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrongShape {
    /// What the caller asked for.
    pub wanted: &'static str,
}

impl fmt::Display for WrongShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "value is not a {}", self.wanted)
    }
}

impl std::error::Error for WrongShape {}

impl Value {
    /// The value as a single string, or `None` if it is a list.
    ///
    /// A single-element list is still a list: the format distinguishes
    /// `a = x` from `a = x,`, and round-tripping depends on keeping them apart.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            Value::List(_) => None,
        }
    }

    /// The value as a list of strings.
    ///
    /// A single value counts as a one-element list, which is usually what a
    /// caller reading a list-valued option wants.
    pub fn as_list(&self) -> Vec<&str> {
        match self {
            Value::String(s) => vec![s.as_str()],
            Value::List(items) => items.iter().map(String::as_str).collect(),
        }
    }

    /// Interpret the value as a boolean.
    ///
    /// Recognises `yes`/`no`, `on`/`off`, `true`/`false` and `1`/`0`, in any
    /// case. Anything else is an error rather than a silent `false`.
    pub fn as_bool(&self) -> Result<bool, NotABool> {
        let text = self.as_str().ok_or(NotABool::WrongShape(WrongShape {
            wanted: "single value",
        }))?;
        match text.to_ascii_lowercase().as_str() {
            "yes" | "on" | "true" | "1" => Ok(true),
            "no" | "off" | "false" | "0" => Ok(false),
            _ => Err(NotABool::Unrecognised(text.to_string())),
        }
    }

    /// Parse the value as any type that knows how to parse itself from a string.
    ///
    /// Useful for numbers: `value.parse::<i64>()`.
    pub fn parse<T>(&self) -> Result<T, ParseValueError<T::Err>>
    where
        T: std::str::FromStr,
    {
        let text = self
            .as_str()
            .ok_or(ParseValueError::WrongShape(WrongShape {
                wanted: "single value",
            }))?;
        text.parse().map_err(ParseValueError::Invalid)
    }
}

/// A failure to parse a value into a requested type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseValueError<E> {
    /// The value was a list, so there was nothing single to parse.
    WrongShape(WrongShape),
    /// The text did not parse as the requested type.
    Invalid(E),
}

impl<E: fmt::Display> fmt::Display for ParseValueError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseValueError::WrongShape(e) => write!(f, "{e}"),
            ParseValueError::Invalid(e) => write!(f, "{e}"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for ParseValueError<E> {}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => f.write_str(s),
            Value::List(items) => f.write_str(&items.join(", ")),
        }
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::String(if b { "True" } else { "False" }.to_string())
    }
}

impl From<Vec<String>> for Value {
    fn from(items: Vec<String>) -> Self {
        Value::List(items)
    }
}

impl From<Vec<&str>> for Value {
    fn from(items: Vec<&str>) -> Self {
        Value::List(items.into_iter().map(str::to_string).collect())
    }
}

macro_rules! value_from_number {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Value {
                fn from(n: $t) -> Self {
                    Value::String(n.to_string())
                }
            }
        )*
    };
}

value_from_number!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);
