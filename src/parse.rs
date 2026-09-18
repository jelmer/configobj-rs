//! Parsing config files into sections.

use crate::error::{parse_error, ErrorKind, ParseError};
use crate::section::{Comments, Entry, Section};
use crate::value::Value;

/// How a file was indented, so writing it back can match.
///
/// The default is no indentation: a file that did not indent its sections is
/// not given indentation when it is written back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Indent(pub String);

pub(crate) struct Parsed {
    pub root: Section,
    pub initial_comment: Vec<String>,
    pub final_comment: Vec<String>,
    pub indent: Option<Indent>,
    pub errors: Vec<ParseError>,
}

/// Settings that change how values are interpreted.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Options {
    /// Split unquoted commas into lists, and unquote values.
    pub list_values: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { list_values: true }
    }
}

pub(crate) fn parse(lines: &[String], options: Options) -> Parsed {
    Parser {
        options,
        errors: Vec::new(),
        indent: None,
    }
    .run(lines)
}

struct Parser {
    options: Options,
    errors: Vec<ParseError>,
    indent: Option<Indent>,
}

/// A section header or key/value pair, recognised but not yet interpreted.
enum Line<'a> {
    Blank,
    Comment,
    Section {
        indent: &'a str,
        depth: usize,
        name: &'a str,
        comment: Option<&'a str>,
    },
    Keyword {
        indent: &'a str,
        key: &'a str,
        rest: &'a str,
    },
    /// Neither a section header nor a `key = value` pair.
    Invalid,
}

impl Parser {
    fn run(mut self, lines: &[String]) -> Parsed {
        let mut root = Section::new();
        // Path of section names from the root down to the section being filled.
        let mut path: Vec<String> = Vec::new();
        let mut pending: Vec<String> = Vec::new();
        let mut initial_comment = Vec::new();
        let mut started = false;

        let mut index = 0;
        while index < lines.len() {
            let raw = &lines[index];
            let line_number = index + 1;

            match self.classify(raw) {
                Line::Invalid => {
                    if !started {
                        initial_comment = std::mem::take(&mut pending);
                        started = true;
                    }
                    self.errors.push(parse_error(
                        ErrorKind::Parse,
                        format!("Invalid line {raw:?} (matched as neither section nor keyword)"),
                        line_number,
                        raw,
                    ));
                    pending.clear();
                    index += 1;
                    continue;
                }
                Line::Blank | Line::Comment => {
                    pending.push(strip_comment_marker(raw));
                    index += 1;
                    continue;
                }
                Line::Section {
                    indent,
                    depth,
                    name,
                    comment,
                } => {
                    if !started {
                        initial_comment = std::mem::take(&mut pending);
                        started = true;
                    }
                    self.note_indent(indent);
                    let name = unquote(name).to_string();
                    let comments = Comments {
                        above: std::mem::take(&mut pending),
                        inline: comment.map(str::to_string),
                    };
                    if let Err(e) = self.place_section(
                        &mut root,
                        &mut path,
                        depth,
                        name,
                        comments,
                        raw,
                        line_number,
                    ) {
                        self.errors.push(e);
                    }
                    index += 1;
                    continue;
                }
                Line::Keyword { indent, key, rest } => {
                    if !started {
                        initial_comment = std::mem::take(&mut pending);
                        started = true;
                    }
                    self.note_indent(indent);

                    let (value, comment, consumed) =
                        match self.read_value(rest, lines, index, line_number) {
                            Ok(parts) => parts,
                            Err(e) => {
                                self.errors.push(e);
                                index += 1;
                                pending.clear();
                                continue;
                            }
                        };
                    index = consumed + 1;

                    let key = unquote(key).to_string();
                    let comments = Comments {
                        above: std::mem::take(&mut pending),
                        inline: comment,
                    };
                    let target = section_at_mut(&mut root, &path);
                    if target.contains_key(&key) {
                        self.errors.push(parse_error(
                            ErrorKind::Duplicate,
                            format!("Duplicate keyword name {key:?}"),
                            line_number,
                            raw,
                        ));
                        continue;
                    }
                    target.insert_parsed(key, Entry::Scalar(value), comments);
                    continue;
                }
            }
        }

        // Trailing comments belong to the file, unless nothing was ever
        // parsed, in which case the whole file is one leading comment.
        let (initial_comment, final_comment) = if started {
            (initial_comment, pending)
        } else {
            (pending, Vec::new())
        };

        Parsed {
            root,
            initial_comment,
            final_comment,
            indent: self.indent,
            errors: self.errors,
        }
    }

    fn note_indent(&mut self, indent: &str) {
        if self.indent.is_none() && !indent.is_empty() {
            self.indent = Some(Indent(indent.to_string()));
        }
    }

    /// Attach a new section at `depth`, adjusting the current path.
    #[allow(clippy::too_many_arguments)]
    fn place_section(
        &mut self,
        root: &mut Section,
        path: &mut Vec<String>,
        depth: usize,
        name: String,
        comments: Comments,
        raw: &str,
        line_number: usize,
    ) -> Result<(), ParseError> {
        // depth 1 is a top-level section, sitting directly under the root.
        if depth > path.len() + 1 {
            return Err(parse_error(
                ErrorKind::Nesting,
                "Section too nested",
                line_number,
                raw,
            ));
        }
        path.truncate(depth - 1);
        let parent = section_at_mut(root, path);
        if parent.contains_key(&name) {
            return Err(parse_error(
                ErrorKind::Duplicate,
                format!("Duplicate section name {name:?}"),
                line_number,
                raw,
            ));
        }
        parent.insert_parsed(name.clone(), Entry::Section(Section::new()), comments);
        path.push(name);
        Ok(())
    }

    fn classify<'a>(&self, raw: &'a str) -> Line<'a> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Line::Blank;
        }
        if trimmed.starts_with('#') {
            return Line::Comment;
        }
        if let Some(section) = match_section(raw) {
            return section;
        }
        match_keyword(raw).unwrap_or(Line::Invalid)
    }

    /// Read a value, following a triple-quoted value across lines.
    ///
    /// Returns the value, any inline comment, and the index of the last line
    /// consumed.
    fn read_value(
        &self,
        rest: &str,
        lines: &[String],
        index: usize,
        line_number: usize,
    ) -> Result<(Value, Option<String>, usize), ParseError> {
        let raw = &lines[index];
        if let Some(quote) = triple_quote_prefix(rest) {
            let (text, comment, last) =
                read_multiline(quote, rest, lines, index).ok_or_else(|| {
                    parse_error(
                        ErrorKind::Parse,
                        "Parse error in multiline value",
                        line_number,
                        raw,
                    )
                })?;
            return Ok((Value::String(text), comment, last));
        }

        let (value, comment) = if self.options.list_values {
            split_value(rest).ok_or_else(|| {
                parse_error(ErrorKind::Parse, "Parse error in value", line_number, raw)
            })?
        } else {
            split_value_no_lists(rest).ok_or_else(|| {
                parse_error(ErrorKind::Parse, "Parse error in value", line_number, raw)
            })?
        };
        Ok((value, comment, index))
    }
}

fn section_at_mut<'a>(root: &'a mut Section, path: &[String]) -> &'a mut Section {
    let mut current = root;
    for name in path {
        current = current
            .section_mut(name)
            .expect("section path is built as sections are created");
    }
    current
}

/// Remove a leading `#` and one following space from a comment line.
fn strip_comment_marker(line: &str) -> String {
    let trimmed = line.trim_start();
    match trimmed.strip_prefix('#') {
        // Drop the marker and at most one space after it, so the text is what
        // the author wrote and writing re-adds the marker exactly once. A
        // second '#' belongs to the comment, not to the marker.
        Some(rest) => rest.strip_prefix(' ').unwrap_or(rest).to_string(),
        None => trimmed.to_string(),
    }
}

/// Match a section header such as `[[name]] # comment`.
fn match_section(line: &str) -> Option<Line<'_>> {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);

    let open = rest.len() - rest.trim_start_matches(['[', ' ', '\t']).len();
    let depth = rest[..open].matches('[').count();
    if depth == 0 {
        return None;
    }
    let body = &rest[open..];

    // The name runs to the last closing bracket on the line, so a '#' before
    // it is part of the name rather than the start of a comment.
    let last_close = body.rfind(']')?;
    let (body, after) = body.split_at(last_close + 1);
    let comment = match after.trim() {
        "" => None,
        rest => Some(rest.strip_prefix('#')?.trim_start()),
    };
    let body = body.trim_end();
    let close_start = body.len() - body.trim_end_matches([']', ' ', '\t']).len();
    let close = &body[body.len() - close_start..];
    if close.matches(']').count() != depth {
        return None;
    }
    let name = body[..body.len() - close_start].trim();
    if name.is_empty() {
        return None;
    }
    Some(Line::Section {
        indent,
        depth,
        name,
        comment,
    })
}

/// Match a `key = value` line, returning the key and everything after the `=`.
fn match_keyword(line: &str) -> Option<Line<'_>> {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);
    if rest.starts_with('=') {
        return None;
    }

    // A quoted key may legitimately contain '='.
    let (key_end, after) =
        if let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') {
            let close = rest[1..].find(quote)? + 1;
            (close + 1, close + 1)
        } else {
            let eq = rest.find('=')?;
            (eq, eq)
        };
    let eq = rest[after..].find('=')? + after;
    let key = rest[..key_end].trim();
    if key.is_empty() {
        return None;
    }
    Some(Line::Keyword {
        indent,
        key,
        rest: rest[eq + 1..].trim_start(),
    })
}

/// The triple-quote a value starts with, if any.
fn triple_quote_prefix(value: &str) -> Option<&'static str> {
    if value.starts_with("\"\"\"") {
        Some("\"\"\"")
    } else if value.starts_with("'''") {
        Some("'''")
    } else {
        None
    }
}

/// Read a triple-quoted value, which may span several lines.
fn read_multiline(
    quote: &str,
    first: &str,
    lines: &[String],
    index: usize,
) -> Option<(String, Option<String>, usize)> {
    let body = &first[quote.len()..];

    // The value may close on the line it opened on.
    if let Some(end) = body.find(quote) {
        let comment = trailing_comment(&body[end + quote.len()..])?;
        return Some((body[..end].to_string(), comment, index));
    }

    let mut text = body.to_string();
    let mut cursor = index;
    while cursor + 1 < lines.len() {
        cursor += 1;
        let line = &lines[cursor];
        match line.find(quote) {
            None => {
                text.push('\n');
                text.push_str(line);
            }
            Some(end) => {
                let comment = trailing_comment(&line[end + quote.len()..])?;
                text.push('\n');
                text.push_str(&line[..end]);
                return Some((text, comment, cursor));
            }
        }
    }
    None
}

/// Interpret the text after a closing quote as an optional comment.
fn trailing_comment(rest: &str) -> Option<Option<String>> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(None);
    }
    let comment = rest.strip_prefix('#')?;
    Some(Some(comment.trim_start().to_string()))
}

/// Split a value into its content and trailing comment.
///
/// A quote only opens a quoted region at the start of a value or list item;
/// elsewhere, as in `it's`, it is an ordinary character. Inside a quoted
/// region a `#` is literal.
fn split_trailing_comment_outside_quotes(value: &str) -> Option<(&str, Option<&str>)> {
    let mut quote: Option<char> = None;
    let mut at_item_start = true;

    for (offset, ch) in value.char_indices() {
        match quote {
            Some(open) => {
                if ch == open {
                    quote = None;
                    at_item_start = false;
                }
            }
            None => match ch {
                '"' | '\'' if at_item_start => quote = Some(ch),
                '#' => {
                    let comment = value[offset + 1..].trim_start();
                    return Some((&value[..offset], Some(comment)));
                }
                ',' => at_item_start = true,
                c if c.is_whitespace() => {}
                _ => at_item_start = false,
            },
        }
    }
    // An unterminated quote is not a usable value.
    if quote.is_some() {
        return None;
    }
    Some((value, None))
}

/// Parse a value with list handling: unquote, and split on unquoted commas.
fn split_value(value: &str) -> Option<(Value, Option<String>)> {
    let (body, comment) = split_trailing_comment_outside_quotes(value)?;
    let comment = comment.map(str::to_string);
    let body = body.trim();

    if body == "," {
        return Some((Value::List(Vec::new()), comment));
    }
    // `a =` with nothing after it is the empty string, not a missing value.
    if body.is_empty() {
        return Some((Value::String(String::new()), comment));
    }

    match split_commas(body)? {
        Items::Single(item) => {
            check_item(item)?;
            Some((Value::String(unquote(item).to_string()), comment))
        }
        Items::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                check_item(item)?;
                out.push(unquote(item).to_string());
            }
            Some((Value::List(out), comment))
        }
    }
}

/// Reject list items that cannot appear in a well-formed value.
///
/// An empty item means a stray or doubled comma, which upstream treats as a
/// parse error rather than an empty string.
fn check_item(item: &str) -> Option<()> {
    if item.is_empty() {
        return None;
    }
    // A leading quote must be closed at the very end of the item.
    if let Some(quote) = item.chars().next().filter(|c| *c == '"' || *c == '\'') {
        if item.len() < 2 || !item.ends_with(quote) {
            return None;
        }
    }
    Some(())
}

/// Parse a written value the way `list_values = false` would, so the writer
/// can check that what it is about to emit reads back unchanged.
///
/// This goes through the whole line, because a value opening with a triple
/// quote is taken as the start of a multiline value before the value rules
/// ever apply.
pub(crate) fn parse_value_no_lists(value: &str) -> Option<String> {
    let lines = vec![format!("k = {value}")];
    let parsed = parse(&lines, Options { list_values: false });
    if !parsed.errors.is_empty() {
        return None;
    }
    parsed.root.get_str("k").map(str::to_string)
}

/// Parse a value without list handling: the text is kept as written, minus a
/// trailing comment.
///
/// A value that starts with a quote must be a single well-formed quoted
/// string, matching what upstream accepts here.
fn split_value_no_lists(value: &str) -> Option<(Value, Option<String>)> {
    let (body, comment) = split_trailing_comment_outside_quotes(value)?;
    let body = body.trim();
    if let Some(quote) = body.chars().next().filter(|c| *c == '"' || *c == '\'') {
        if body.len() < 2 || !body.ends_with(quote) {
            return None;
        }
    }
    Some((Value::String(body.to_string()), comment.map(str::to_string)))
}

enum Items<'a> {
    Single(&'a str),
    List(Vec<&'a str>),
}

/// Split on commas that are not inside quotes.
///
/// A trailing comma marks a list, so `a,` is a one-element list while `a` is a
/// plain value.
fn split_commas(body: &str) -> Option<Items<'_>> {
    let mut parts = Vec::new();
    let mut quote: Option<char> = None;
    let mut at_item_start = true;
    let mut start = 0;
    let mut saw_comma = false;

    for (offset, ch) in body.char_indices() {
        match quote {
            Some(open) => {
                if ch == open {
                    quote = None;
                    at_item_start = false;
                }
            }
            None => match ch {
                '"' | '\'' if at_item_start => quote = Some(ch),
                ',' => {
                    parts.push(body[start..offset].trim());
                    start = offset + 1;
                    saw_comma = true;
                    at_item_start = true;
                }
                c if c.is_whitespace() => {}
                _ => at_item_start = false,
            },
        }
    }
    if quote.is_some() {
        return None;
    }

    if !saw_comma {
        return Some(Items::Single(body));
    }
    let last = body[start..].trim();
    if !last.is_empty() {
        parts.push(last);
    }
    Some(Items::List(parts))
}

/// Strip one layer of matching quotes.
pub(crate) fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        if (first == b'"' || first == b'\'') && bytes[bytes.len() - 1] == first {
            return &value[1..value.len() - 1];
        }
    }
    value
}
