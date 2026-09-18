//! Checks that this crate reads config files the way the Python configobj does.
//!
//! The expectations in `data/reference.txt` were recorded from the Python
//! implementation once and committed, so these tests need nothing installed
//! and cannot be perturbed by whatever happens to be on the machine. See
//! `tests/README.md` for how to regenerate them.

use configobj::{Builder, ConfigObj, Section, Value};

include!("shared/corpus.rs");

/// The recorded expectations, kept next to this file.
const REFERENCE: &str = include_str!("data/reference.txt");

/// One recorded answer: what Python made of an input under a list-values setting.
struct Expectation {
    input: String,
    list_values: bool,
    result: &'static str,
}

/// Read the reference file, skipping its comment header.
fn expectations() -> Vec<Expectation> {
    REFERENCE
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let mut fields = line.split('\t');
            let input = fields.next().expect("every record has an input");
            let list_values = fields.next().expect("every record has a list flag");
            let result = fields.next().expect("every record has a result");
            Expectation {
                input: json_unescape(input),
                list_values: list_values == "1",
                result,
            }
        })
        .collect()
}

/// Decode a JSON string literal, which is how inputs are stored.
fn json_unescape(literal: &str) -> String {
    let body = literal
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .expect("inputs are stored as JSON strings");
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next().expect("an escape is never left dangling") {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                let code = u32::from_str_radix(&hex, 16).expect("a four digit escape");
                out.push(char::from_u32(code).expect("a valid scalar value"));
            }
            other => panic!("unsupported escape \\{other}"),
        }
    }
    out
}

/// Describe a parse result the way the reference file records it.
fn describe(section: &Section) -> String {
    let entries: Vec<String> = section
        .keys()
        .map(|key| match (section.get(key), section.section(key)) {
            (Some(Value::String(s)), _) => {
                format!("[{}, \"string\", {}]", json_string(key), json_string(s))
            }
            (Some(Value::List(items)), _) => {
                let items: Vec<String> = items.iter().map(|i| json_string(i)).collect();
                format!("[{}, \"list\", [{}]]", json_string(key), items.join(", "))
            }
            (_, Some(child)) => {
                format!("[{}, \"section\", {}]", json_string(key), describe(child))
            }
            _ => unreachable!("every key holds either a value or a section"),
        })
        .collect();
    format!("[{}]", entries.join(", "))
}

#[test]
fn parsing_matches_the_reference() {
    let expectations = expectations();
    assert!(
        !expectations.is_empty(),
        "the reference file should hold expectations"
    );

    let mut mismatches = Vec::new();
    for expected in &expectations {
        let parsed = Builder::new()
            .list_values(expected.list_values)
            .from_str(&expected.input);
        let actual = match &parsed {
            Ok(conf) => describe(conf.root()),
            Err(_) => "\"ERROR\"".to_string(),
        };
        if actual != expected.result {
            let input = &expected.input;
            let list_values = expected.list_values;
            let reference = expected.result;
            mismatches.push(format!(
                "input {input:?} (list_values={list_values})\n  \
                 reference: {reference}\n  ours:      {actual}"
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} inputs parsed differently from the reference:\n{}",
        mismatches.len(),
        expectations.len(),
        mismatches.join("\n")
    );
}

#[test]
fn the_reference_covers_the_whole_corpus() {
    // A corpus entry with no recorded answer would be silently untested.
    let recorded: std::collections::HashSet<(String, bool)> = expectations()
        .into_iter()
        .map(|e| (e.input, e.list_values))
        .collect();

    let missing: Vec<String> = corpus()
        .into_iter()
        .filter(|text| {
            !recorded.contains(&(text.clone(), true)) || !recorded.contains(&(text.clone(), false))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "{} corpus input(s) have no recorded answer, so tests/data/reference.txt \
         needs regenerating: {missing:?}",
        missing.len()
    );
}

#[test]
fn values_we_write_are_read_back_unchanged() {
    // Whatever the writer emits has to parse back into the value that was
    // stored, including the forms upstream's writer gets wrong.
    let values: Vec<Value> = vec![
        Value::String("plain".into()),
        Value::String(String::new()),
        Value::String("with space".into()),
        Value::String("  padded  ".into()),
        Value::String("has#hash".into()),
        Value::String("has,comma".into()),
        Value::String("it's".into()),
        Value::String("say \"hi\"".into()),
        Value::String("both ' and \"".into()),
        Value::String("line\nbreak".into()),
        Value::String("triple ''' inside".into()),
        Value::String("triple \"\"\" inside".into()),
        Value::String("spam\n\"\"\" that's my spam \"\"\"\neggs".into()),
        Value::String("=equals".into()),
        Value::String("[brackets]".into()),
        Value::List(vec![]),
        Value::List(vec!["one".into()]),
        Value::List(vec!["one".into(), "two".into()]),
        Value::List(vec!["with space".into(), "with,comma".into()]),
    ];

    let mut failures = Vec::new();
    for value in &values {
        let mut conf = ConfigObj::new();
        conf.insert("key", value.clone());
        let text = match conf.to_string() {
            Ok(text) => text,
            Err(e) => {
                failures.push(format!("{value:?}: could not write: {e}"));
                continue;
            }
        };
        match ConfigObj::from_str(&text) {
            Ok(back) if back.get("key") == Some(value) => {}
            Ok(back) => failures.push(format!(
                "{value:?}: wrote {text:?}, read back {:?}",
                back.get("key")
            )),
            Err(e) => failures.push(format!("{value:?}: wrote {text:?}, could not read it: {e}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} value(s) did not survive being written:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn a_file_in_the_upstream_style_is_read_correctly() {
    // The shape the Python implementation writes, covering the forms its
    // writer produces.
    let text = "\
# a header
plain = value
spaced = \"  padded  \"
listed = a, b c
single = only,
empty_list = ,
hash = \"has#hash\"
quote = \"it's\"
[section]
nested = yes
[[deeper]]
x = 1
# a footer
";
    let conf = ConfigObj::from_str(text).expect("should parse");
    assert_eq!(conf.get_str("plain"), Some("value"));
    assert_eq!(conf.get_str("spaced"), Some("  padded  "));
    assert_eq!(conf.get_list("listed"), Some(vec!["a", "b c"]));
    assert_eq!(conf.get("single"), Some(&Value::List(vec!["only".into()])));
    assert_eq!(conf.get("empty_list"), Some(&Value::List(vec![])));
    assert_eq!(conf.get_str("hash"), Some("has#hash"));
    assert_eq!(conf.get_str("quote"), Some("it's"));
    let section = conf.section("section").expect("nested section");
    assert_eq!(section.get_str("nested"), Some("yes"));
    assert_eq!(
        section.section("deeper").and_then(|s| s.get_str("x")),
        Some("1")
    );
    assert_eq!(conf.initial_comment(), ["a header".to_string()]);
    assert_eq!(conf.final_comment(), ["a footer".to_string()]);
}
