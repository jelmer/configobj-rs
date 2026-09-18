//! Tests that writing and re-reading a config preserves it.

use configobj::{Builder, ConfigObj, Section, Value};

/// Write a config out and read it back.
fn cycle(conf: &ConfigObj) -> ConfigObj {
    let text = conf.to_string().expect("should write");
    ConfigObj::from_str(&text).unwrap_or_else(|e| panic!("could not re-read {text:?}: {e}"))
}

/// Check that a value survives being written and read back.
fn survives(value: Value) {
    let mut conf = ConfigObj::new();
    conf.insert("key", value.clone());
    let text = conf.to_string().expect("should write");
    let back = cycle(&conf);
    assert_eq!(
        back.get("key"),
        Some(&value),
        "value did not survive being written as {text:?}"
    );
}

#[test]
fn plain_values_survive() {
    for text in ["x", "with space", "x=y", "x:y", "x/y", "3", "true"] {
        survives(Value::String(text.to_string()));
    }
}

#[test]
fn the_empty_string_survives() {
    survives(Value::String(String::new()));
}

#[test]
fn values_with_surrounding_space_survive() {
    survives(Value::String("  padded  ".to_string()));
    survives(Value::String("\ttabbed\t".to_string()));
}

#[test]
fn values_with_hashes_survive() {
    survives(Value::String("has#hash".to_string()));
    survives(Value::String("#leading".to_string()));
    survives(Value::String("trailing#".to_string()));
}

#[test]
fn values_with_commas_survive() {
    survives(Value::String("has,comma".to_string()));
    survives(Value::String(",leading".to_string()));
}

#[test]
fn values_with_quotes_survive() {
    survives(Value::String("it's".to_string()));
    survives(Value::String("say \"hi\"".to_string()));
    survives(Value::String("both ' and \"".to_string()));
}

#[test]
fn multiline_values_survive() {
    survives(Value::String("one\ntwo".to_string()));
    survives(Value::String("one\ntwo\nthree".to_string()));
}

#[test]
fn values_containing_triple_quotes_survive() {
    // This is the case upstream configobj gets wrong (lp:710410): it writes a
    // file it cannot read back.
    survives(Value::String(
        "spam\n\"\"\" that's my spam \"\"\"\neggs".to_string(),
    ));
    survives(Value::String("has ''' inside\nand a newline".to_string()));
}

#[test]
fn a_value_with_both_kinds_of_triple_quote_cannot_be_written() {
    let mut conf = ConfigObj::new();
    conf.insert("key", "\"\"\" and '''\nnewline");
    let err = conf.to_string().expect_err("no quote would work here");
    assert!(matches!(err, configobj::Error::Unwritable(_)));
}

#[test]
fn lists_survive() {
    survives(Value::List(vec![]));
    survives(Value::List(vec!["one".to_string()]));
    survives(Value::List(vec!["one".to_string(), "two".to_string()]));
    survives(Value::List(vec![
        "with space".to_string(),
        "with,comma".to_string(),
    ]));
    survives(Value::List(vec![
        "it's".to_string(),
        "say \"hi\"".to_string(),
    ]));
    survives(Value::List(vec![String::new(), "after empty".to_string()]));
}

#[test]
fn a_single_item_list_stays_a_list() {
    let mut conf = ConfigObj::new();
    conf.insert("key", Value::List(vec!["only".to_string()]));
    assert_eq!(conf.to_string().expect("write"), "key = only,\n");
    assert_eq!(
        cycle(&conf).get("key"),
        Some(&Value::List(vec!["only".to_string()])),
        "the trailing comma is what keeps this a list"
    );
}

#[test]
fn structure_survives() {
    let mut conf = ConfigObj::new();
    conf.insert("top", "1");
    let section = conf.entry_section("outer");
    section.insert("mid", "2");
    section.entry_section("inner").insert("deep", "3");

    let back = cycle(&conf);
    assert_eq!(back.get_str("top"), Some("1"));
    let outer = back.section("outer").expect("outer");
    assert_eq!(outer.get_str("mid"), Some("2"));
    assert_eq!(
        outer.section("inner").and_then(|s| s.get_str("deep")),
        Some("3")
    );
}

#[test]
fn key_order_survives() {
    let mut conf = ConfigObj::new();
    for key in ["zebra", "apple", "mango"] {
        conf.insert(key, "x");
    }
    assert_eq!(
        cycle(&conf).keys().collect::<Vec<_>>(),
        vec!["zebra", "apple", "mango"],
        "entries should stay where the author put them"
    );
}

#[test]
fn awkward_keys_survive() {
    for key in ["with space", "with=equals", "with#hash", "with,comma"] {
        let mut conf = ConfigObj::new();
        conf.insert(key, "x");
        let text = conf.to_string().expect("should write");
        let back = ConfigObj::from_str(&text)
            .unwrap_or_else(|e| panic!("could not re-read {text:?}: {e}"));
        assert_eq!(back.get_str(key), Some("x"), "wrote {text:?}");
    }
}

#[test]
fn awkward_section_names_survive() {
    for name in [
        "with space",
        "name *.txt",
        "with#hash",
        "a/b/c",
        "http://example.com/",
    ] {
        let mut conf = ConfigObj::new();
        conf.entry_section(name).insert("x", "1");
        let text = conf.to_string().expect("should write");
        let back = ConfigObj::from_str(&text)
            .unwrap_or_else(|e| panic!("could not re-read {text:?}: {e}"));
        assert_eq!(
            back.section(name).and_then(|s| s.get_str("x")),
            Some("1"),
            "wrote {text:?}"
        );
    }
}

#[test]
fn comments_survive() {
    let text =
        "# header\n\na = 1\n# about b\nb = 2 # inline b\n\n# about s\n[s]\nc = 3\n# footer\n";
    let conf = ConfigObj::from_str(text).expect("should parse");
    let written = conf.to_string().expect("should write");
    let back = ConfigObj::from_str(&written).expect("should re-read");

    assert_eq!(back.initial_comment(), conf.initial_comment());
    assert_eq!(back.final_comment(), conf.final_comment());
    assert_eq!(back.comments("b"), conf.comments("b"));
    assert_eq!(back.comments("s"), conf.comments("s"));
}

#[test]
fn a_parsed_file_is_stable_when_rewritten() {
    // Writing an unmodified file a second time should not keep changing it.
    let inputs = [
        "a = x\n",
        "a = x, y\n",
        "[s]\na = x\n[[t]]\nb = y\n",
        "# header\na = x\n# footer\n",
        "a = x # comment\n",
        "a = \"\"\"one\ntwo\"\"\"\n",
    ];
    for text in inputs {
        let first = ConfigObj::from_str(text)
            .unwrap_or_else(|e| panic!("could not parse {text:?}: {e}"))
            .to_string()
            .expect("should write");
        let second = ConfigObj::from_str(&first)
            .unwrap_or_else(|e| panic!("could not re-parse {first:?}: {e}"))
            .to_string()
            .expect("should write");
        assert_eq!(first, second, "rewriting {text:?} was not stable");
    }
}

#[test]
fn indentation_is_reused_when_rewriting() {
    let conf = ConfigObj::from_str("[s]\n  a = x\n").expect("should parse");
    assert_eq!(conf.indent(), "  ");
    assert_eq!(conf.to_string().expect("write"), "[s]\n  a = x\n");
}

#[test]
fn a_config_with_no_indentation_is_written_flat() {
    let mut conf = ConfigObj::new();
    conf.entry_section("a").entry_section("b").insert("x", "1");
    assert_eq!(conf.to_string().expect("write"), "[a]\n[[b]]\nx = 1\n");
}

#[test]
fn observed_indentation_is_applied_per_level() {
    let conf = ConfigObj::from_str("[a]\n  [[b]]\n    x = 1\n").expect("should parse");
    assert_eq!(conf.indent(), "  ");
    assert_eq!(
        conf.to_string().expect("write"),
        "[a]\n  [[b]]\n    x = 1\n"
    );
}

#[test]
fn list_values_off_round_trips_text_as_written() {
    // This is how breezy's config stores use the crate: values are kept as
    // written and quoted by the caller.
    let text = "a = \"quoted\"\nb = x, y\n";
    let conf = Builder::new()
        .list_values(false)
        .from_str(text)
        .expect("should parse");
    assert_eq!(conf.to_string().expect("write"), text);
}

#[test]
fn a_config_built_by_hand_writes_cleanly() {
    let mut conf = ConfigObj::new();
    conf.set_initial_comment(["written by hand"]);
    conf.insert("name", "Jelmer");
    conf.insert("tags", Value::List(vec!["a".into(), "b".into()]));
    let mut section = Section::new();
    section.insert("nested", "yes");
    conf.insert_section("s", section);

    assert_eq!(
        conf.to_string().expect("write"),
        "# written by hand\nname = Jelmer\ntags = a, b\n[s]\nnested = yes\n"
    );
}

#[test]
fn scalars_are_written_before_sections() {
    // A key written after a section header would be read back inside that
    // section, so the writer has to order them.
    let mut conf = ConfigObj::new();
    conf.entry_section("s").insert("inner", "1");
    conf.insert("outer", "2");

    let back = cycle(&conf);
    assert_eq!(back.get_str("outer"), Some("2"));
    assert_eq!(
        back.section("s").and_then(|s| s.get_str("inner")),
        Some("1")
    );
}

#[test]
fn files_round_trip_through_disk() {
    let dir = std::env::temp_dir().join(format!("configobj-rs-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("test.ini");

    let mut conf = ConfigObj::new();
    conf.insert("a", "x");
    conf.entry_section("s").insert("b", "y");
    conf.write_file(&path).expect("write file");

    let back = ConfigObj::from_file(&path).expect("read file");
    assert_eq!(back.get_str("a"), Some("x"));
    assert_eq!(back.section("s").and_then(|s| s.get_str("b")), Some("y"));
    assert_eq!(back.filename(), Some(path.as_path()));

    std::fs::remove_dir_all(&dir).expect("clean up");
}

#[test]
fn saving_and_reloading_uses_the_remembered_file() {
    let dir = std::env::temp_dir().join(format!("configobj-rs-reload-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("test.ini");
    std::fs::write(&path, "a = x\n").expect("seed file");

    let mut conf = ConfigObj::from_file(&path).expect("read file");
    conf.insert("b", "y");
    conf.save().expect("save");

    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        "a = x\nb = y\n"
    );

    conf.insert("c", "z");
    conf.reload().expect("reload");
    assert_eq!(conf.get_str("c"), None, "reload should discard changes");
    assert_eq!(conf.get_str("b"), Some("y"));

    std::fs::remove_dir_all(&dir).expect("clean up");
}

#[test]
fn saving_without_a_filename_is_an_error() {
    let conf = ConfigObj::new();
    assert!(conf.save().is_err());
}

#[test]
fn blank_separator_lines_are_not_indented() {
    // Indenting a blank line would leave trailing whitespace in the file.
    let text = "[editor]\ncommand = vi\n\n    [[options]]\n    tabs = no\n";
    let conf = ConfigObj::from_str(text).expect("should parse");
    let written = conf.to_string().expect("should write");
    assert!(
        !written
            .lines()
            .any(|l| !l.is_empty() && l.trim().is_empty()),
        "wrote a line of only whitespace:\n{written}"
    );
}

#[test]
fn reloading_keeps_the_reading_options() {
    let dir = std::env::temp_dir().join(format!("configobj-rs-opts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("test.ini");
    std::fs::write(&path, "a = x, y\n").expect("seed file");

    let mut conf = Builder::new()
        .list_values(false)
        .from_file(&path)
        .expect("read file");
    conf.set_indent("  ");
    conf.reload().expect("reload");

    assert_eq!(
        conf.get_str("a"),
        Some("x, y"),
        "reload should keep list_values off"
    );
    assert_eq!(conf.indent(), "  ", "reload should keep the indent setting");

    std::fs::remove_dir_all(&dir).expect("clean up");
}

#[test]
fn a_comment_starting_with_a_hash_keeps_both() {
    // The stored text is what follows the marker, so a comment reading
    // "#more" must not come back with one hash eaten.
    let conf = ConfigObj::from_str("##double\nk = v\n").expect("should parse");
    assert_eq!(conf.initial_comment(), ["#double".to_string()]);
    let written = conf.to_string().expect("should write");
    assert_eq!(
        ConfigObj::from_str(&written)
            .expect("should re-read")
            .initial_comment(),
        ["#double".to_string()],
        "wrote {written:?}"
    );
}

#[test]
fn comment_text_survives_being_rewritten() {
    for text in ["# a\nk = v\n", "# a\n\n# b\nk = v\n", "k = v # tail\n"] {
        let conf = ConfigObj::from_str(text).expect("should parse");
        let written = conf.to_string().expect("should write");
        assert_eq!(written, text, "rewriting {text:?} changed it");
    }
}
