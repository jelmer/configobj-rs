//! Turning sections back into config file text.

use crate::error::{Error, Result};
use crate::parse::Indent;
use crate::section::{Entry, Section};
use crate::value::Value;

/// Characters that force a value to be quoted when they start or end it.
const WSPACE_PLUS: &[char] = &[' ', '\r', '\n', '\u{b}', '\t', '\'', '"'];

pub(crate) struct Writer<'a> {
    pub indent: &'a Indent,
    pub list_values: bool,
}

impl Writer<'_> {
    pub fn write(
        &self,
        root: &Section,
        initial_comment: &[String],
        final_comment: &[String],
    ) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for line in initial_comment {
            out.push(comment_line(line));
        }
        self.write_section(root, 0, &mut out)?;
        for line in final_comment {
            out.push(comment_line(line));
        }
        Ok(out)
    }

    fn write_section(&self, section: &Section, depth: usize, out: &mut Vec<String>) -> Result<()> {
        let indent = self.indent.0.repeat(depth);

        // Scalars come before subsections: once a section header is written,
        // any following key would land inside that subsection.
        for key in section.scalars() {
            self.write_comments(section, key, &indent, out);
            let value = section
                .get(key)
                .expect("scalars() only yields keys holding scalars");
            out.push(format!(
                "{indent}{} = {}{}",
                self.quote_key(key)?,
                self.quote_value(value)?,
                self.inline_comment(section, key),
            ));
        }

        for name in section.section_names() {
            self.write_comments(section, name, &indent, out);
            let marker = "[".repeat(depth + 1);
            let close = "]".repeat(depth + 1);
            out.push(format!(
                "{indent}{marker}{}{close}{}",
                self.quote_section_name(name)?,
                self.inline_comment(section, name),
            ));
            let child = match section.entry(name) {
                Some(Entry::Section(s)) => s,
                _ => unreachable!("section_names() only yields keys holding sections"),
            };
            self.write_section(child, depth + 1, out)?;
        }
        Ok(())
    }

    fn write_comments(&self, section: &Section, key: &str, indent: &str, out: &mut Vec<String>) {
        let Some(comments) = section.comments(key) else {
            return;
        };
        for line in &comments.above {
            let text = comment_line(line);
            // A blank separator line is left blank rather than indented into
            // a line of trailing whitespace.
            out.push(if text.is_empty() {
                text
            } else {
                format!("{indent}{text}")
            });
        }
    }

    fn inline_comment(&self, section: &Section, key: &str) -> String {
        match section.comments(key).and_then(|c| c.inline.as_deref()) {
            Some(comment) => format!(" # {comment}"),
            None => String::new(),
        }
    }

    /// Quote a value as it would appear in a file, for callers assembling
    /// config text by hand.
    pub fn quote_value_for_api(&self, value: &Value) -> Result<String> {
        self.quote_value(value)
    }

    /// Quote a key.
    ///
    /// A key holding an `=` has to be quoted or reading the line back would
    /// split it at the wrong place; upstream configobj misses this and loses
    /// the tail of such a key.
    fn quote_key(&self, key: &str) -> Result<String> {
        let quoted = self.quote(key, false)?;
        if quoted == key && key.contains('=') {
            return self.single_quote(key);
        }
        Ok(quoted)
    }

    /// Quote a section name.
    ///
    /// A name holding a bracket would confuse the section header, so it is
    /// quoted even when a value with the same text would not be.
    fn quote_section_name(&self, name: &str) -> Result<String> {
        let quoted = self.quote(name, false)?;
        if quoted == name && (name.contains('[') || name.contains(']')) {
            return self.single_quote(name);
        }
        Ok(quoted)
    }

    fn quote_value(&self, value: &Value) -> Result<String> {
        match value {
            Value::String(s) => self.quote(s, true),
            Value::List(items) => match items.split_first() {
                // An empty list is written as a bare comma, and a single-item
                // list keeps a trailing comma, so both survive a round trip.
                None => Ok(",".to_string()),
                Some((only, [])) => Ok(format!("{},", self.quote(only, false)?)),
                Some(_) => {
                    let parts: Result<Vec<_>> =
                        items.iter().map(|i| self.quote(i, false)).collect();
                    Ok(parts?.join(", "))
                }
            },
        }
    }

    /// Quote a value so that reading it back yields the same text.
    ///
    /// `multiline` allows triple quotes, which suit values holding newlines or
    /// both kinds of quote. List items are written with `multiline` off, since
    /// a list cannot span lines.
    fn quote(&self, value: &str, multiline: bool) -> Result<String> {
        if value.is_empty() {
            return Ok("\"\"".to_string());
        }

        // With list values off, nothing is quoted on the way out because
        // nothing is unquoted on the way in: the caller owns any quoting. A
        // value that would not read back as itself is refused rather than
        // written wrong, where upstream writes `a = a#b` and reads back `a`.
        if !self.list_values {
            if !reads_back_unchanged(value) {
                return Err(Error::Unwritable(value.to_string()));
            }
            return Ok(value.to_string());
        }

        let has_single = value.contains('\'');
        let has_double = value.contains('"');
        let needs_triple = multiline && ((has_single && has_double) || value.contains('\n'));

        if needs_triple {
            return self.triple_quote(value);
        }

        if value.contains('\n') {
            // Only reachable for list items and keys, which cannot span lines.
            return Err(Error::Unwritable(value.to_string()));
        }

        // A value opening with a triple quote would be read as the start of a
        // multiline value, so it needs quoting even when nothing else forces it.
        if opens_triple_quote(value) {
            return self.triple_quote(value);
        }

        let starts_or_ends_awkwardly =
            value.starts_with(WSPACE_PLUS) || value.ends_with(WSPACE_PLUS);
        let bare = !starts_or_ends_awkwardly && !value.contains(',') && !value.contains('#');
        if bare {
            return Ok(value.to_string());
        }
        self.single_quote(value)
    }

    fn single_quote(&self, value: &str) -> Result<String> {
        match (value.contains('\''), value.contains('"')) {
            (true, true) => Err(Error::Unwritable(value.to_string())),
            (false, true) => Ok(format!("'{value}'")),
            _ => Ok(format!("\"{value}\"")),
        }
    }

    /// Wrap the value in a triple quote it can actually be read back through.
    ///
    /// A quote is usable when it does not appear in the value and does not
    /// touch the value's own first or last character, since `"""x""""` would
    /// be ambiguous. Upstream configobj picks the wrong quote here (lp:710410)
    /// and writes files it cannot read back.
    fn triple_quote(&self, value: &str) -> Result<String> {
        for quote in ["\"\"\"", "'''"] {
            let mark = quote.chars().next().expect("candidates are non-empty");
            let collides =
                value.contains(quote) || value.starts_with(mark) || value.ends_with(mark);
            if !collides {
                return Ok(format!("{quote}{value}{quote}"));
            }
        }
        Err(Error::Unwritable(value.to_string()))
    }
}

/// Render a comment line, or a blank line for a blank separator.
///
/// The stored text is what the author wrote after the marker, so the marker is
/// re-added here unconditionally: a comment reading `#more` keeps both hashes,
/// and an empty comment stays a comment rather than becoming a blank line.
fn comment_line(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    format!("# {text}")
}

/// Whether a value written verbatim would parse back as itself.
///
/// With list values off the writer adds no quoting, so a `#` outside quotes
/// would be read as the start of a comment. This asks the parser rather than
/// restating its rules.
fn reads_back_unchanged(value: &str) -> bool {
    crate::parse::parse_value_no_lists(value).is_some_and(|parsed| parsed == value)
}

/// Whether a value would be read as opening a triple-quoted value.
fn opens_triple_quote(value: &str) -> bool {
    value.starts_with("\"\"\"") || value.starts_with("'''")
}
