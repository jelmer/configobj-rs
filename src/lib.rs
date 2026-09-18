//! Reader and writer for the ConfigObj configuration file format.
//!
//! The format is INI-like, with a few additions: sections nest by repeating
//! the brackets (`[[child]]`), values may be comma-separated lists, and
//! comments are attached to the entries they precede so that rewriting a file
//! leaves it looking the way its author wrote it.
//!
//! ```
//! use configobj::ConfigObj;
//!
//! let conf = ConfigObj::from_str(
//!     "name = Jelmer\n\
//!      languages = rust, python\n\
//!      \n\
//!      [editor]\n\
//!      command = vi\n",
//! )?;
//!
//! assert_eq!(conf.get_str("name"), Some("Jelmer"));
//! assert_eq!(conf.get_list("languages"), Some(vec!["rust", "python"]));
//! assert_eq!(
//!     conf.section("editor").and_then(|s| s.get_str("command")),
//!     Some("vi"),
//! );
//! # Ok::<(), configobj::Error>(())
//! ```
//!
//! Values are text until you ask for something else, which mirrors the file
//! format itself:
//!
//! ```
//! # use configobj::ConfigObj;
//! let conf = ConfigObj::from_str("debug = yes\nport = 8080\n")?;
//! assert_eq!(conf.get_bool("debug").transpose()?, Some(true));
//! assert_eq!(conf.get("port").unwrap().parse::<u16>().ok(), Some(8080));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![deny(missing_docs)]

mod error;
mod parse;
mod section;
mod value;
mod write;

pub use error::{Error, ErrorKind, ParseError, Result};
pub use section::{Comments, Section};
pub use value::{NotABool, ParseValueError, Value, WrongShape};

use parse::Indent;
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

/// The byte order mark that marks a file as UTF-8.
const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Options for reading a config file.
///
/// ```
/// use configobj::Builder;
///
/// // Keep values exactly as written, without splitting lists or unquoting.
/// let conf = Builder::new().list_values(false).from_str("a = x, y\n")?;
/// assert_eq!(conf.get_str("a"), Some("x, y"));
/// # Ok::<(), configobj::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct Builder {
    list_values: bool,
}

impl Default for Builder {
    fn default() -> Self {
        Builder { list_values: true }
    }
}

impl Builder {
    /// Start from the default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether to split unquoted commas into lists and strip quotes.
    ///
    /// With this off, a value is the text as written, minus any trailing
    /// comment. That suits callers that do their own quoting.
    pub fn list_values(mut self, enabled: bool) -> Self {
        self.list_values = enabled;
        self
    }

    /// Parse config text.
    pub fn from_str(&self, text: &str) -> Result<ConfigObj> {
        self.build(split_lines(text), None)
    }

    /// Parse config text from bytes, honouring a UTF-8 byte order mark.
    pub fn from_bytes(&self, bytes: &[u8]) -> Result<ConfigObj> {
        let (text, bom) = decode(bytes)?;
        let mut conf = self.build(split_lines(&text), None)?;
        conf.bom = bom;
        Ok(conf)
    }

    /// Parse a sequence of lines, which may have trailing newlines or not.
    pub fn from_lines<I, S>(&self, lines: I) -> Result<ConfigObj>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let lines = lines
            .into_iter()
            .map(|l| l.as_ref().trim_end_matches(['\r', '\n']).to_string())
            .collect();
        self.build(lines, None)
    }

    /// Read and parse a file.
    ///
    /// A missing file is an error; use [`ConfigObj::new`] for an empty config.
    pub fn from_file(&self, path: impl AsRef<Path>) -> Result<ConfigObj> {
        let path = path.as_ref();
        let mut bytes = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut bytes)?;
        let (text, bom) = decode(&bytes)?;
        let mut conf = self.build(split_lines(&text), Some(path.to_path_buf()))?;
        conf.bom = bom;
        Ok(conf)
    }

    /// Read and parse from any reader.
    pub fn from_reader(&self, mut reader: impl Read) -> Result<ConfigObj> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        self.from_bytes(&bytes)
    }

    fn build(&self, lines: Vec<String>, filename: Option<PathBuf>) -> Result<ConfigObj> {
        let options = parse::Options {
            list_values: self.list_values,
        };
        let parsed = parse::parse(&lines, options);
        if !parsed.errors.is_empty() {
            return Err(Error::Parse(parsed.errors));
        }
        Ok(ConfigObj {
            root: parsed.root,
            initial_comment: parsed.initial_comment,
            final_comment: parsed.final_comment,
            indent: parsed.indent.unwrap_or_default(),
            list_values: self.list_values,
            filename,
            bom: false,
        })
    }
}

/// A parsed config file: the top-level section, plus the details needed to
/// write it back out the way it came in.
///
/// It derefs to [`Section`], so the whole section API works on the top level
/// of the file directly.
#[derive(Debug, Clone)]
pub struct ConfigObj {
    root: Section,
    initial_comment: Vec<String>,
    final_comment: Vec<String>,
    indent: Indent,
    list_values: bool,
    filename: Option<PathBuf>,
    bom: bool,
}

impl Default for ConfigObj {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigObj {
    /// An empty config with default settings.
    pub fn new() -> Self {
        ConfigObj {
            root: Section::new(),
            initial_comment: Vec::new(),
            final_comment: Vec::new(),
            indent: Indent::default(),
            list_values: true,
            filename: None,
            bom: false,
        }
    }

    /// Parse config text with default options.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Result<Self> {
        Builder::new().from_str(text)
    }

    /// Parse config bytes with default options, honouring a UTF-8 byte order mark.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Builder::new().from_bytes(bytes)
    }

    /// Parse a sequence of lines with default options.
    pub fn from_lines<I, S>(lines: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Builder::new().from_lines(lines)
    }

    /// Read and parse a file with default options.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        Builder::new().from_file(path)
    }

    /// Read and parse from any reader with default options.
    pub fn from_reader(reader: impl Read) -> Result<Self> {
        Builder::new().from_reader(reader)
    }

    /// The top-level section.
    pub fn root(&self) -> &Section {
        &self.root
    }

    /// Mutable access to the top-level section.
    pub fn root_mut(&mut self) -> &mut Section {
        &mut self.root
    }

    /// The file this config was read from, if any.
    pub fn filename(&self) -> Option<&Path> {
        self.filename.as_deref()
    }

    /// Set the file this config is associated with, which is where
    /// [`ConfigObj::save`] and [`ConfigObj::reload`] act.
    pub fn set_filename(&mut self, path: impl Into<PathBuf>) {
        self.filename = Some(path.into());
    }

    /// Comment lines at the very top of the file.
    pub fn initial_comment(&self) -> &[String] {
        &self.initial_comment
    }

    /// Replace the comment lines at the top of the file.
    pub fn set_initial_comment<I, S>(&mut self, lines: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.initial_comment = lines.into_iter().map(Into::into).collect();
    }

    /// Comment lines at the very bottom of the file.
    pub fn final_comment(&self) -> &[String] {
        &self.final_comment
    }

    /// Replace the comment lines at the bottom of the file.
    pub fn set_final_comment<I, S>(&mut self, lines: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.final_comment = lines.into_iter().map(Into::into).collect();
    }

    /// The indentation used for nested sections.
    pub fn indent(&self) -> &str {
        &self.indent.0
    }

    /// Set the indentation used for nested sections.
    pub fn set_indent(&mut self, indent: impl Into<String>) {
        self.indent = Indent(indent.into());
    }

    /// Whether a UTF-8 byte order mark was seen, and so will be written back.
    pub fn has_bom(&self) -> bool {
        self.bom
    }

    /// Choose whether to write a UTF-8 byte order mark.
    pub fn set_bom(&mut self, bom: bool) {
        self.bom = bom;
    }

    /// Render the config as lines, without trailing newlines.
    pub fn to_lines(&self) -> Result<Vec<String>> {
        write::Writer {
            indent: &self.indent,
            list_values: self.list_values,
        }
        .write(&self.root, &self.initial_comment, &self.final_comment)
    }

    /// Render the config as text.
    ///
    /// The result ends with a newline when it is not empty.
    pub fn to_string(&self) -> Result<String> {
        let lines = self.to_lines()?;
        if lines.is_empty() {
            return Ok(String::new());
        }
        let mut text = lines.join("\n");
        text.push('\n');
        Ok(text)
    }

    /// Render the config as bytes, including a byte order mark if one is set.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let text = self.to_string()?;
        let mut bytes = Vec::with_capacity(text.len() + BOM_UTF8.len());
        if self.bom {
            bytes.extend_from_slice(BOM_UTF8);
        }
        bytes.extend_from_slice(text.as_bytes());
        Ok(bytes)
    }

    /// Write the config to a writer.
    pub fn write(&self, mut writer: impl Write) -> Result<()> {
        writer.write_all(&self.to_bytes()?)?;
        Ok(())
    }

    /// Write the config to a file.
    pub fn write_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.to_bytes()?;
        std::fs::write(path.as_ref(), bytes)?;
        Ok(())
    }

    /// Write the config back to the file it was read from.
    ///
    /// Errors if this config has no associated file.
    pub fn save(&self) -> Result<()> {
        let path = self.filename.as_deref().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "config has no filename to save to",
            ))
        })?;
        self.write_file(path)
    }

    /// Re-read the config from the file it was read from, discarding changes.
    pub fn reload(&mut self) -> Result<()> {
        let path = self.filename.clone().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "config has no filename to reload from",
            ))
        })?;
        let mut fresh = Builder::new()
            .list_values(self.list_values)
            .from_file(&path)?;
        // A file with no indentation of its own keeps whatever the caller set.
        if fresh.indent.0.is_empty() {
            fresh.indent = self.indent.clone();
        }
        *self = fresh;
        Ok(())
    }

    /// Quote a value so that parsing it yields the value back.
    ///
    /// This is for callers that keep values quoted themselves, typically
    /// alongside [`Builder::list_values(false)`](Builder::list_values): the
    /// value is quoted here and stored as-is, and [`ConfigObj::unquote`]
    /// returns the original. Quoting always follows the list-value rules,
    /// since those are what make a value safe to read back.
    pub fn quote(value: &str) -> Result<String> {
        write::Writer {
            indent: &Indent::default(),
            list_values: true,
        }
        .quote_for_api(value)
    }

    /// Strip one layer of matching quotes from a value, as parsing would.
    pub fn unquote(value: &str) -> &str {
        parse::unquote(value)
    }
}

impl Deref for ConfigObj {
    type Target = Section;

    fn deref(&self) -> &Section {
        &self.root
    }
}

impl DerefMut for ConfigObj {
    fn deref_mut(&mut self) -> &mut Section {
        &mut self.root
    }
}

impl std::str::FromStr for ConfigObj {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        ConfigObj::from_str(text)
    }
}

/// Split text into lines, dropping line endings.
fn split_lines(text: &str) -> Vec<String> {
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect()
}

/// Decode bytes as UTF-8, reporting whether a byte order mark was present.
fn decode(bytes: &[u8]) -> Result<(String, bool)> {
    let (body, bom) = match bytes.strip_prefix(BOM_UTF8) {
        Some(rest) => (rest, true),
        None => (bytes, false),
    };
    let text = std::str::from_utf8(body)
        .map_err(|e| Error::Encoding(format!("input is not valid UTF-8: {e}")))?;
    Ok((text.to_string(), bom))
}
