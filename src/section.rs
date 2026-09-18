//! Sections: ordered maps of keys and subsections.

use crate::value::Value;
use std::collections::HashMap;

/// Comments attached to an entry.
///
/// `above` holds whole lines preceding the entry (each without its leading
/// `#`, which is re-added on writing). `inline` is the trailing comment on the
/// entry's own line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comments {
    /// Comment lines sitting immediately above the entry.
    pub above: Vec<String>,
    /// Trailing comment on the same line as the entry.
    pub inline: Option<String>,
}

impl Comments {
    /// True if there is no comment text at all.
    pub fn is_empty(&self) -> bool {
        self.above.is_empty() && self.inline.is_none()
    }
}

/// An entry in a section, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entry {
    Scalar(Value),
    Section(Section),
}

/// A section of a config file: an ordered map of keys to values, plus nested
/// subsections.
///
/// Insertion order is preserved, so rewriting a file leaves entries where the
/// author put them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    order: Vec<String>,
    entries: HashMap<String, Entry>,
    comments: HashMap<String, Comments>,
}

impl Section {
    /// Create an empty section.
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of entries, counting scalars and subsections together.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// True if the section holds nothing at all.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// True if `key` names either a value or a subsection here.
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// Every key in this section, in file order, including subsection names.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.order.iter().map(String::as_str)
    }

    /// The names of the scalar entries, in file order.
    pub fn scalars(&self) -> impl Iterator<Item = &str> {
        self.order.iter().filter_map(move |k| {
            matches!(self.entries.get(k), Some(Entry::Scalar(_))).then_some(k.as_str())
        })
    }

    /// The names of the subsections, in file order.
    pub fn section_names(&self) -> impl Iterator<Item = &str> {
        self.order.iter().filter_map(move |k| {
            matches!(self.entries.get(k), Some(Entry::Section(_))).then_some(k.as_str())
        })
    }

    /// The value stored under `key`, if it is a scalar.
    ///
    /// Returns `None` when `key` is absent or names a subsection.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self.entries.get(key) {
            Some(Entry::Scalar(v)) => Some(v),
            _ => None,
        }
    }

    /// Mutable access to the value under `key`, if it is a scalar.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self.entries.get_mut(key) {
            Some(Entry::Scalar(v)) => Some(v),
            _ => None,
        }
    }

    /// The value under `key` as a string, if it is a single (non-list) value.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// The value under `key` interpreted as a boolean.
    ///
    /// `None` if the key is absent; `Some(Err(..))` if it is present but not a
    /// recognised boolean, so a typo is never silently treated as `false`.
    pub fn get_bool(&self, key: &str) -> Option<Result<bool, crate::value::NotABool>> {
        self.get(key).map(Value::as_bool)
    }

    /// The value under `key` as a list.
    ///
    /// A single value counts as a one-element list.
    pub fn get_list(&self, key: &str) -> Option<Vec<&str>> {
        self.get(key).map(Value::as_list)
    }

    /// The subsection named `key`, if there is one.
    pub fn section(&self, key: &str) -> Option<&Section> {
        match self.entries.get(key) {
            Some(Entry::Section(s)) => Some(s),
            _ => None,
        }
    }

    /// Mutable access to the subsection named `key`.
    pub fn section_mut(&mut self, key: &str) -> Option<&mut Section> {
        match self.entries.get_mut(key) {
            Some(Entry::Section(s)) => Some(s),
            _ => None,
        }
    }

    /// Look up a value by a path of section names ending in a key.
    ///
    /// `conf.get_path(&["auth", "example.com", "user"])` walks two sections
    /// down. An empty path yields `None`.
    pub fn get_path(&self, path: &[&str]) -> Option<&Value> {
        let (key, sections) = path.split_last()?;
        let mut current = self;
        for name in sections {
            current = current.section(name)?;
        }
        current.get(key)
    }

    /// Look up a nested subsection by path.
    pub fn section_path(&self, path: &[&str]) -> Option<&Section> {
        let mut current = self;
        for name in path {
            current = current.section(name)?;
        }
        Some(current)
    }

    /// Store a scalar value, replacing whatever was there.
    ///
    /// A new key goes at the end; an existing one keeps its position and its
    /// comments. Replacing a subsection with a scalar discards the subsection.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        let key = key.into();
        self.set_entry(key, Entry::Scalar(value.into()));
    }

    /// Store a subsection, replacing whatever was there.
    pub fn insert_section(&mut self, key: impl Into<String>, section: Section) {
        let key = key.into();
        self.set_entry(key, Entry::Section(section));
    }

    fn set_entry(&mut self, key: String, entry: Entry) {
        if !self.entries.contains_key(&key) {
            self.order.push(key.clone());
        }
        self.entries.insert(key, entry);
    }

    /// The subsection named `key`, creating an empty one if needed.
    ///
    /// If `key` currently holds a scalar, it is replaced by a new section.
    pub fn entry_section(&mut self, key: impl Into<String>) -> &mut Section {
        let key = key.into();
        if !matches!(self.entries.get(&key), Some(Entry::Section(_))) {
            self.set_entry(key.clone(), Entry::Section(Section::new()));
        }
        match self.entries.get_mut(&key) {
            Some(Entry::Section(s)) => s,
            // set_entry above guarantees a section is present.
            _ => unreachable!("entry_section just installed a section"),
        }
    }

    /// Remove the value or subsection under `key`, returning true if
    /// something was removed.
    pub fn remove(&mut self, key: &str) -> bool {
        if self.entries.remove(key).is_none() {
            return false;
        }
        self.order.retain(|k| k != key);
        self.comments.remove(key);
        true
    }

    /// Remove every entry.
    pub fn clear(&mut self) {
        self.order.clear();
        self.entries.clear();
        self.comments.clear();
    }

    /// Rename an entry, keeping its position, value and comments.
    ///
    /// Returns false if `from` does not exist, or if `to` is already taken by
    /// a different entry.
    pub fn rename(&mut self, from: &str, to: impl Into<String>) -> bool {
        let to = to.into();
        if from == to {
            return self.entries.contains_key(from);
        }
        if !self.entries.contains_key(from) || self.entries.contains_key(&to) {
            return false;
        }
        let entry = self
            .entries
            .remove(from)
            .expect("presence checked immediately above");
        self.entries.insert(to.clone(), entry);
        if let Some(comments) = self.comments.remove(from) {
            self.comments.insert(to.clone(), comments);
        }
        let slot = self
            .order
            .iter_mut()
            .find(|s| *s == from)
            .expect("an entry that exists has a slot in the order");
        *slot = to;
        true
    }

    /// Iterate over the scalar entries in file order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.order
            .iter()
            .filter_map(move |k| match self.entries.get(k) {
                Some(Entry::Scalar(v)) => Some((k.as_str(), v)),
                _ => None,
            })
    }

    /// Iterate over the subsections in file order.
    pub fn sections(&self) -> impl Iterator<Item = (&str, &Section)> {
        self.order
            .iter()
            .filter_map(move |k| match self.entries.get(k) {
                Some(Entry::Section(s)) => Some((k.as_str(), s)),
                _ => None,
            })
    }

    /// The comments attached to `key`.
    pub fn comments(&self, key: &str) -> Option<&Comments> {
        self.comments.get(key)
    }

    /// Mutable access to the comments attached to `key`, creating an empty set
    /// if the entry exists but has no comments yet.
    ///
    /// Returns `None` if there is no such entry to comment on.
    pub fn comments_mut(&mut self, key: &str) -> Option<&mut Comments> {
        if !self.entries.contains_key(key) {
            return None;
        }
        Some(self.comments.entry(key.to_string()).or_default())
    }

    /// Attach comments to an existing entry, returning false if there is none.
    pub fn set_comments(&mut self, key: &str, comments: Comments) -> bool {
        if !self.entries.contains_key(key) {
            return false;
        }
        self.comments.insert(key.to_string(), comments);
        true
    }

    pub(crate) fn entry(&self, key: &str) -> Option<&Entry> {
        self.entries.get(key)
    }

    pub(crate) fn insert_parsed(&mut self, key: String, entry: Entry, comments: Comments) {
        self.set_entry(key.clone(), entry);
        if !comments.is_empty() {
            self.comments.insert(key, comments);
        }
    }
}

impl<'a> IntoIterator for &'a Section {
    type Item = (&'a str, &'a Value);
    type IntoIter = Box<dyn Iterator<Item = (&'a str, &'a Value)> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl<K, V> Extend<(K, V)> for Section
where
    K: Into<String>,
    V: Into<Value>,
{
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<K, V> FromIterator<(K, V)> for Section
where
    K: Into<String>,
    V: Into<Value>,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut section = Section::new();
        section.extend(iter);
        section
    }
}
