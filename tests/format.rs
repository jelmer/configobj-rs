//! Tests for file format compatibility with the Python configobj.

use configobj::{Builder, ConfigObj, Value};

fn parse(text: &str) -> ConfigObj {
    ConfigObj::from_str(text).expect("should parse")
}

#[test]
fn simple_values() {
    let conf = parse("name = Jelmer\nempty =\n");
    assert_eq!(conf.get_str("name"), Some("Jelmer"));
    assert_eq!(conf.get_str("empty"), Some(""));
}

#[test]
fn values_keep_inner_whitespace_but_lose_surrounding() {
    let conf = parse("a =   x y   \n");
    assert_eq!(conf.get_str("a"), Some("x y"));
}

#[test]
fn quoted_values_keep_surrounding_whitespace() {
    let conf = parse("a = \"  x  \"\n");
    assert_eq!(conf.get_str("a"), Some("  x  "));
}

#[test]
fn single_and_double_quotes_are_stripped() {
    let conf = parse("a = 'x'\nb = \"y\"\n");
    assert_eq!(conf.get_str("a"), Some("x"));
    assert_eq!(conf.get_str("b"), Some("y"));
}

#[test]
fn quotes_inside_a_value_are_kept() {
    let conf = parse("a = \"it's here\"\nb = 'say \"hi\"'\n");
    assert_eq!(conf.get_str("a"), Some("it's here"));
    assert_eq!(conf.get_str("b"), Some("say \"hi\""));
}

#[test]
fn lists_split_on_commas() {
    let conf = parse("a = x, y, z\n");
    assert_eq!(conf.get_list("a"), Some(vec!["x", "y", "z"]));
}

#[test]
fn a_trailing_comma_makes_a_one_item_list() {
    let conf = parse("a = x,\n");
    assert_eq!(conf.get("a"), Some(&Value::List(vec!["x".to_string()])));
}

#[test]
fn a_bare_comma_is_an_empty_list() {
    let conf = parse("a = ,\n");
    assert_eq!(conf.get("a"), Some(&Value::List(vec![])));
}

#[test]
fn a_value_without_commas_is_not_a_list() {
    let conf = parse("a = x\n");
    assert_eq!(conf.get("a"), Some(&Value::String("x".to_string())));
}

#[test]
fn commas_inside_quotes_do_not_split() {
    let conf = parse("a = \"x, y\"\n");
    assert_eq!(conf.get_str("a"), Some("x, y"));
}

#[test]
fn list_items_are_unquoted_individually() {
    let conf = parse("a = 'x x', \"y y\"\n");
    assert_eq!(conf.get_list("a"), Some(vec!["x x", "y y"]));
}

#[test]
fn comments_are_stripped_from_values() {
    let conf = parse("a = x # a comment\n");
    assert_eq!(conf.get_str("a"), Some("x"));
    assert_eq!(
        conf.comments("a").and_then(|c| c.inline.as_deref()),
        Some("a comment")
    );
}

#[test]
fn a_hash_inside_quotes_is_not_a_comment() {
    let conf = parse("a = \"x # y\"\n");
    assert_eq!(conf.get_str("a"), Some("x # y"));
}

#[test]
fn sections_nest_by_bracket_depth() {
    let conf = parse("[a]\nx = 1\n[[b]]\ny = 2\n[[[c]]]\nz = 3\n");
    let a = conf.section("a").expect("section a");
    assert_eq!(a.get_str("x"), Some("1"));
    let b = a.section("b").expect("section b");
    assert_eq!(b.get_str("y"), Some("2"));
    let c = b.section("c").expect("section c");
    assert_eq!(c.get_str("z"), Some("3"));
}

#[test]
fn a_shallower_section_closes_deeper_ones() {
    let conf = parse("[a]\n[[b]]\n[c]\nx = 1\n");
    assert!(conf.section("a").expect("a").section("b").is_some());
    assert_eq!(
        conf.section("c").expect("c").get_str("x"),
        Some("1"),
        "[c] should be top level again, not nested under [a]"
    );
}

#[test]
fn section_names_are_unquoted() {
    let conf = parse("['a b']\nx = 1\n");
    assert!(conf.section("a b").is_some());
}

#[test]
fn section_names_keep_inner_spacing() {
    let conf = parse("[name *.txt *.py]\nx = 1\n");
    assert!(
        conf.section("name *.txt *.py").is_some(),
        "breezy's .bzrrules relies on section names surviving verbatim"
    );
}

#[test]
fn indentation_does_not_affect_nesting() {
    let conf = parse("[a]\n    x = 1\n    [[b]]\n        y = 2\n");
    let a = conf.section("a").expect("a");
    assert_eq!(a.get_str("x"), Some("1"));
    assert_eq!(a.section("b").expect("b").get_str("y"), Some("2"));
}

#[test]
fn triple_quoted_values_span_lines() {
    let conf = parse("a = \"\"\"one\ntwo\"\"\"\n");
    assert_eq!(conf.get_str("a"), Some("one\ntwo"));
}

#[test]
fn triple_quoted_values_may_close_on_the_same_line() {
    let conf = parse("a = '''x'''\n");
    assert_eq!(conf.get_str("a"), Some("x"));
}

#[test]
fn a_triple_quoted_value_may_contain_the_other_quote() {
    let conf = parse("a = \"\"\"it's '''fine'''\"\"\"\n");
    assert_eq!(conf.get_str("a"), Some("it's '''fine'''"));
}

#[test]
fn a_comment_may_follow_a_triple_quoted_value() {
    let conf = parse("a = '''x\ny''' # note\n");
    assert_eq!(conf.get_str("a"), Some("x\ny"));
    assert_eq!(
        conf.comments("a").and_then(|c| c.inline.as_deref()),
        Some("note")
    );
}

#[test]
fn keys_may_be_quoted_and_contain_equals() {
    let conf = parse("'a = b' = x\n");
    assert_eq!(conf.get_str("a = b"), Some("x"));
}

#[test]
fn a_value_may_contain_equals() {
    let conf = parse("a = x=y\n");
    assert_eq!(conf.get_str("a"), Some("x=y"));
}

#[test]
fn comments_above_an_entry_are_kept() {
    let conf = parse("a = 1\n# about b\nb = 2\n");
    let comments = conf.comments("b").expect("comments on b");
    assert_eq!(comments.above, vec!["about b".to_string()]);
}

#[test]
fn a_comment_block_at_the_top_belongs_to_the_file() {
    // Even with no blank line separating it from the first entry, the leading
    // block is the file header rather than a comment on that entry.
    let conf = parse("# one\n# two\na = x\n");
    assert_eq!(
        conf.initial_comment(),
        ["one".to_string(), "two".to_string()]
    );
    assert!(conf.comments("a").is_none_or(|c| c.above.is_empty()));
}

#[test]
fn the_leading_comment_keeps_its_blank_lines() {
    let conf = parse("# header\n\na = x\n");
    assert_eq!(
        conf.initial_comment(),
        ["header".to_string(), String::new()]
    );
}

#[test]
fn comments_attach_to_the_section_they_precede() {
    let conf = parse("a = 1\n\n# about sect\n[sect]\n# about c\nc = 3 # inline c\n");
    assert_eq!(
        conf.comments("sect").expect("comments on sect").above,
        vec![String::new(), "about sect".to_string()]
    );
    let sect = conf.section("sect").expect("sect");
    assert_eq!(
        sect.comments("c").expect("comments on c").above,
        vec!["about c".to_string()]
    );
    assert_eq!(
        sect.comments("c").and_then(|c| c.inline.as_deref()),
        Some("inline c")
    );
}

#[test]
fn the_trailing_comment_belongs_to_the_file() {
    let conf = parse("a = x\n# footer\n");
    assert_eq!(conf.final_comment(), ["footer".to_string()]);
}

#[test]
fn booleans_are_recognised_in_any_case() {
    let conf = parse("a = true\nb = False\nc = YES\nd = off\ne = 1\nf = 0\n");
    for (key, expected) in [
        ("a", true),
        ("b", false),
        ("c", true),
        ("d", false),
        ("e", true),
        ("f", false),
    ] {
        assert_eq!(
            conf.get_bool(key).expect("present").expect("a boolean"),
            expected,
            "{key}"
        );
    }
}

#[test]
fn a_non_boolean_is_an_error_not_false() {
    let conf = parse("a = maybe\n");
    assert!(conf.get_bool("a").expect("present").is_err());
}

#[test]
fn numbers_parse_on_request() {
    let conf = parse("port = 8080\n");
    assert_eq!(conf.get("port").expect("port").parse::<u16>(), Ok(8080));
}

#[test]
fn list_values_off_keeps_the_text_as_written() {
    let conf = Builder::new()
        .list_values(false)
        .from_str("a = x, y\nb = \"quoted\"\n")
        .expect("should parse");
    assert_eq!(conf.get_str("a"), Some("x, y"));
    assert_eq!(
        conf.get_str("b"),
        Some("\"quoted\""),
        "with list_values off, quotes are left in place"
    );
}

#[test]
fn a_duplicate_key_is_an_error_naming_its_line() {
    let err = ConfigObj::from_str("a = 1\na = 2\n").expect_err("duplicate key");
    assert_eq!(err.line_number(), Some(2));
    assert_eq!(err.parse_errors()[0].kind, configobj::ErrorKind::Duplicate);
}

#[test]
fn a_duplicate_section_is_an_error_naming_its_line() {
    // This is breezy's TestConfigObjErrors.test_duplicate_section_name_error_line.
    let text =
        "[section] # line 1\ngood=good # line 2\n[section] # line 3\nwhocares=notme # line 4\n";
    let err = ConfigObj::from_str(text).expect_err("duplicate section");
    assert_eq!(err.line_number(), Some(3));
}

#[test]
fn an_unparsable_line_is_an_error() {
    let err = ConfigObj::from_str("this is not valid\n").expect_err("bad line");
    assert_eq!(err.parse_errors()[0].kind, configobj::ErrorKind::Parse);
}

#[test]
fn an_overly_nested_section_is_an_error() {
    let err = ConfigObj::from_str("[[a]]\n").expect_err("too nested");
    assert_eq!(err.parse_errors()[0].kind, configobj::ErrorKind::Nesting);
}

#[test]
fn an_empty_file_parses_to_nothing() {
    let conf = parse("");
    assert!(conf.is_empty());
    assert_eq!(conf.to_string().expect("write"), "");
}

#[test]
fn a_utf8_bom_is_consumed_and_restored() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"a = x\n");
    let conf = ConfigObj::from_bytes(&bytes).expect("should parse");
    assert_eq!(conf.get_str("a"), Some("x"));
    assert!(conf.has_bom());
    assert_eq!(conf.to_bytes().expect("write"), bytes);
}

#[test]
fn non_ascii_values_survive() {
    let conf = parse("name = Jelmer Vernoo\u{133}\n");
    assert_eq!(conf.get_str("name"), Some("Jelmer Vernoo\u{133}"));
}

#[test]
fn invalid_utf8_is_reported() {
    let err = ConfigObj::from_bytes(b"a = \xff\xfe\n").expect_err("bad encoding");
    assert!(matches!(err, configobj::Error::Encoding(_)));
}

#[test]
fn crlf_line_endings_are_accepted() {
    let conf = parse("a = x\r\n[s]\r\nb = y\r\n");
    assert_eq!(conf.get_str("a"), Some("x"));
    assert_eq!(conf.section("s").expect("s").get_str("b"), Some("y"));
}

#[test]
fn lines_may_be_supplied_individually() {
    let conf = ConfigObj::from_lines(["a = x", "[s]", "b = y"]).expect("should parse");
    assert_eq!(conf.get_str("a"), Some("x"));
    assert_eq!(conf.section("s").expect("s").get_str("b"), Some("y"));
}
