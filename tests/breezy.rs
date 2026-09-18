//! The behaviour breezy relies on, taken from its own tests and config code.
//!
//! Breezy reads and writes bazaar.conf, locations.conf, branch.conf,
//! authentication.conf and .bzrrules with configobj, so these cases stand in
//! for a port of that code.

use configobj::{Builder, ConfigObj, Value};

const BOOL_CONFIG: &str = "\
[DEFAULT]
active = true
inactive = false
[UPPERCASE]
active = True
nonactive = False
";

#[test]
fn booleans_read_as_breezy_expects() {
    // breezy.tests.test_config.TestConfigObj.test_get_bool
    let conf = ConfigObj::from_str(BOOL_CONFIG).expect("should parse");
    let get = |section: &str, key: &str| {
        conf.section(section)
            .and_then(|s| s.get_bool(key))
            .expect("key should be present")
            .expect("should be a boolean")
    };
    assert!(get("DEFAULT", "active"));
    assert!(!get("DEFAULT", "inactive"));
    assert!(get("UPPERCASE", "active"));
    assert!(!get("UPPERCASE", "nonactive"));
}

#[test]
fn a_hash_in_a_value_is_quoted_on_write() {
    // breezy.tests.test_config.TestConfigObj.test_hash_sign_in_value, which
    // guards against the value being read back as a comment (bz #86838).
    let mut conf = ConfigObj::new();
    conf.insert("test", "foo#bar");
    let text = conf.to_string().expect("should write");
    assert_eq!(text, "test = \"foo#bar\"\n");

    let back = ConfigObj::from_str(&text).expect("should re-read");
    assert_eq!(back.get_str("test"), Some("foo#bar"));
}

#[test]
fn triple_quoted_values_round_trip() {
    // breezy.tests.test_config.TestConfigObj.test_triple_quotes, for lp:710410.
    let value = "spam\n\"\"\" that's my spam \"\"\"\neggs";
    let mut conf = ConfigObj::new();
    conf.insert("test", value);

    let text = conf.to_string().expect("should write");
    let back =
        ConfigObj::from_str(&text).unwrap_or_else(|e| panic!("could not re-read {text:?}: {e}"));
    assert_eq!(back.get_str("test"), Some(value));
}

#[test]
fn a_duplicate_section_reports_its_line_number() {
    // breezy.tests.test_config.TestConfigObjErrors
    let text = "\
[section] # line 1
good=good # line 2
[section] # line 3
whocares=notme # line 4
";
    let err = ConfigObj::from_str(text).expect_err("duplicate section");
    assert_eq!(err.line_number(), Some(3));
}

#[test]
fn a_parse_error_carries_the_offending_line() {
    // breezy turns these into ParseConfigError, showing the message to the user.
    let err = ConfigObj::from_str("[section]\nnot a valid line\n").expect_err("bad line");
    let first = &err.parse_errors()[0];
    assert_eq!(first.line_number, 2);
    assert_eq!(first.line, "not a valid line");
}

#[test]
fn bzrrules_sections_keep_their_names() {
    // breezy.rules reads .bzrrules and re-splits the section name itself, so
    // the name has to survive parsing untouched.
    let text = "\
[name *.txt]
foo = bar
[name *.txt *.py 'x x' \"y y\"]
baz = qux
[name ./a.txt]
a = b
";
    let conf = ConfigObj::from_str(text).expect("should parse");
    let names: Vec<&str> = conf.section_names().collect();
    assert_eq!(
        names,
        vec![
            "name *.txt",
            "name *.txt *.py 'x x' \"y y\"",
            "name ./a.txt"
        ]
    );
}

#[test]
fn locations_config_keeps_urls_as_section_names() {
    // breezy.config.LocationConfig uses paths and URLs as section names, and
    // treats a trailing slash as a distinct section.
    let text = "\
[http://example.com/branch]
a = 1
[http://example.com/branch/]
b = 2
[/home/jelmer/src/x]
c = 3
";
    let conf = ConfigObj::from_str(text).expect("should parse");
    assert_eq!(
        conf.section("http://example.com/branch")
            .and_then(|s| s.get_str("a")),
        Some("1")
    );
    assert_eq!(
        conf.section("http://example.com/branch/")
            .and_then(|s| s.get_str("b")),
        Some("2")
    );
    assert_eq!(
        conf.section("/home/jelmer/src/x")
            .and_then(|s| s.get_str("c")),
        Some("3")
    );
}

#[test]
fn authentication_config_reads_as_one_level_of_sections() {
    // breezy.config.AuthenticationConfig expects every top-level entry to be a
    // section, and reads a port as an integer.
    let text = "\
[example]
scheme = https
host = example.com
port = 8080
user = jelmer
verify_certificates = no
";
    let conf = ConfigObj::from_str(text).expect("should parse");
    assert_eq!(conf.scalars().count(), 0, "no values outside a section");

    let auth = conf.section("example").expect("example section");
    assert_eq!(auth.get_str("host"), Some("example.com"));
    assert_eq!(
        auth.get("port").expect("port").parse::<u16>(),
        Ok(8080),
        "breezy reads the port with as_int"
    );
    assert!(!auth
        .get_bool("verify_certificates")
        .expect("present")
        .expect("a boolean"));
}

#[test]
fn a_value_outside_any_section_is_visible_at_the_top_level() {
    // breezy.config.IniFileStore treats top-level scalars as an unnamed section.
    let conf = ConfigObj::from_str("email = Jelmer\n[s]\na = 1\n").expect("should parse");
    assert_eq!(conf.scalars().collect::<Vec<_>>(), vec!["email"]);
    assert_eq!(conf.section_names().collect::<Vec<_>>(), vec!["s"]);
}

#[test]
fn sections_are_iterated_in_file_order() {
    // breezy.config.IniFileStore documents that it wants file order.
    let conf = ConfigObj::from_str("[zebra]\n[apple]\n[mango]\n").expect("should parse");
    assert_eq!(
        conf.section_names().collect::<Vec<_>>(),
        vec!["zebra", "apple", "mango"]
    );
}

#[test]
fn list_values_off_leaves_quoting_to_the_caller() {
    // breezy.config.IniFileStore parses with list_values off and does its own
    // quoting, so these forms have to survive untouched.
    let conf = Builder::new()
        .list_values(false)
        .from_str("a = \" a b c \"\nb = \" a , b c \"\nc = \",\"\n")
        .expect("should parse");
    assert_eq!(conf.get_str("a"), Some("\" a b c \""));
    assert_eq!(conf.get_str("b"), Some("\" a , b c \""));
    assert_eq!(conf.get_str("c"), Some("\",\""));
}

#[test]
fn quoting_and_unquoting_are_inverses_for_awkward_values() {
    // breezy.config quotes values itself before storing them with
    // list_values off, then unquotes them on the way out.
    for value in [
        "\" a b c \"",
        "\" a , b c \"",
        "\",\"",
        "plain",
        "with space",
    ] {
        assert_eq!(
            ConfigObj::unquote(&ConfigObj::quote(value).expect("should quote")),
            value,
            "quoting {value:?} should be reversible"
        );
    }
}

#[test]
fn lists_are_parsed_the_way_breezy_converts_them() {
    // breezy.config's list converter feeds a synthetic "list=<value>" line
    // through configobj to borrow its list splitting.
    let cases: Vec<(&str, Value)> = vec![
        ("a, b", Value::List(vec!["a".into(), "b".into()])),
        ("a,", Value::List(vec!["a".into()])),
        (",", Value::List(vec![])),
        ("a", Value::String("a".into())),
        ("'a, b'", Value::String("a, b".into())),
        ("\" a \", b", Value::List(vec![" a ".into(), "b".into()])),
    ];
    for (input, expected) in cases {
        let conf = ConfigObj::from_str(&format!("list = {input}\n"))
            .unwrap_or_else(|e| panic!("could not parse list = {input}: {e}"));
        assert_eq!(conf.get("list"), Some(&expected), "input {input:?}");
    }
}

#[test]
fn config_is_read_from_lines_of_bytes() {
    // breezy.bzr.remote parses a config transferred over the smart protocol,
    // which arrives as a list of byte lines.
    let body: &[u8] = b"[DEFAULT]\nemail = Jelmer\n";
    let lines: Vec<String> = body
        .split(|b| *b == b'\n')
        .map(|l| String::from_utf8(l.to_vec()).expect("utf-8"))
        .collect();
    let conf = ConfigObj::from_lines(lines).expect("should parse");
    assert_eq!(
        conf.section("DEFAULT").and_then(|s| s.get_str("email")),
        Some("Jelmer")
    );
}

#[test]
fn a_typical_bazaar_conf_round_trips() {
    let text = "\
[DEFAULT]
email = Jelmer Vernoo\u{133} <jelmer@example.com>
editor = vi
gpg_signing_key = AABBCCDD
check_signatures = check-available
create_signatures = when-required
";
    let conf = ConfigObj::from_str(text).expect("should parse");
    assert_eq!(conf.to_string().expect("should write"), text);
}

#[test]
fn editing_a_config_leaves_the_rest_alone() {
    // breezy rewrites config files in place, so an edit should not disturb
    // unrelated entries or their comments.
    let text = "\
# my config
[DEFAULT]
# who I am
email = Jelmer
editor = vi
";
    let mut conf = ConfigObj::from_str(text).expect("should parse");
    conf.section_mut("DEFAULT")
        .expect("DEFAULT")
        .insert("editor", "emacs");

    assert_eq!(
        conf.to_string().expect("should write"),
        "\
# my config
[DEFAULT]
# who I am
email = Jelmer
editor = emacs
"
    );
}

#[test]
fn removing_an_option_removes_its_comments() {
    let mut conf = ConfigObj::from_str("[s]\n# about a\na = 1\nb = 2\n").expect("should parse");
    let section = conf.section_mut("s").expect("s");
    assert!(section.remove("a"));
    assert_eq!(conf.to_string().expect("should write"), "[s]\nb = 2\n");
}

#[test]
fn a_missing_section_is_created_on_demand() {
    // breezy uses setdefault(section, {}) when setting an option.
    let mut conf = ConfigObj::new();
    conf.entry_section("DEFAULT").insert("email", "Jelmer");
    assert_eq!(
        conf.to_string().expect("should write"),
        "[DEFAULT]\nemail = Jelmer\n"
    );
}

#[test]
fn a_hash_cannot_be_written_when_quoting_is_left_to_the_caller() {
    // With list values off nothing is quoted on the way out, because nothing
    // is unquoted on the way in, so a bare '#' would be read back as the start
    // of a comment. Upstream writes it anyway and silently loses the tail.
    let mut conf = Builder::new()
        .list_values(false)
        .from_str("a = z\n")
        .expect("should parse");
    conf.insert("a", "a#b");
    assert!(matches!(
        conf.to_string(),
        Err(configobj::Error::Unwritable(_))
    ));
}

#[test]
fn a_caller_quoted_hash_survives_with_list_values_off() {
    // The caller quotes it themselves, which is how breezy's stores work.
    let mut conf = Builder::new()
        .list_values(false)
        .from_str("a = z\n")
        .expect("should parse");
    let quoted = ConfigObj::quote("a#b").expect("should quote");
    conf.insert("a", quoted.clone());

    let text = conf.to_string().expect("should write");
    let back = Builder::new()
        .list_values(false)
        .from_str(&text)
        .expect("should re-read");
    assert_eq!(back.get_str("a"), Some(quoted.as_str()));
    assert_eq!(ConfigObj::unquote(quoted.as_str()), "a#b");
}
