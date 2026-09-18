// The shared corpus of config inputs used for compatibility checking, and the
// JSON helper that describes a parse result. Included by both the
// compatibility test and the example that regenerates the reference file, so
// the two cannot drift apart.

/// Every input the two implementations should agree on, hand-written cases
/// first and mechanically generated ones after.
fn corpus() -> Vec<String> {
    let mut all: Vec<String> = handwritten().into_iter().map(String::from).collect();
    all.extend(generated_corpus());
    all
}

/// Inputs chosen to pin down a specific rule of the format.
fn handwritten() -> Vec<&'static str> {
    vec![
        "",
        "a = x\n",
        "a = x\nb = y\n",
        "a =\n",
        "a = \"\"\n",
        "a = ''\n",
        "a = x, y\n",
        "a = x,\n",
        "a = ,\n",
        "a = x, y, z\n",
        "a = 'x x', \"y y\"\n",
        "a = \"x, y\"\n",
        "a = x # comment\n",
        "a = \"x # y\"\n",
        "a = '#'\n",
        "a =   spaced   \n",
        "a = \"  spaced  \"\n",
        "a = it's\n",
        "a = 'say \"hi\"'\n",
        "a = \"it's\"\n",
        "a = x=y\n",
        "'quoted key' = x\n",
        "\"dq key\" = x\n",
        "a = '''triple'''\n",
        "a = \"\"\"triple\"\"\"\n",
        "a = \"\"\"one\ntwo\"\"\"\n",
        "a = '''one\ntwo\nthree'''\n",
        "a = '''x''' # note\n",
        "[s]\n",
        "[s]\na = x\n",
        "[s]\na = x\n[t]\nb = y\n",
        "[s]\n[[t]]\na = x\n",
        "[s]\n[[t]]\n[[[u]]]\na = x\n",
        "[s]\n[[t]]\na = x\n[s2]\nb = y\n",
        "[s]\n    a = x\n    [[t]]\n        b = y\n",
        "['quoted section']\na = x\n",
        "[name *.txt *.py]\na = x\n",
        "[a b]\nx = 1\n",
        "# header\na = x\n",
        "# header\n\na = x\n",
        "a = x\n# footer\n",
        "# only a comment\n",
        "\n\n",
        "a = x\n\n\nb = y\n",
        "[s] # section comment\na = x # value comment\n",
        "a = 1\n# about b\nb = 2\n",
        "a = True\nb = false\n",
        "a = 0\nb = 1\n",
        "a = -1\nb = 3.5\n",
        "a = x y z\n",
        "a = ,x\n",
        "a = x,,y\n",
        "a = \"a\", \"b\",\n",
        "a = [notalist]\n",
        "a = %(b)s\nb = y\n",
        "a = $b\n",
        // Values that look structural but are not.
        "a = ]\n",
        "a = [\n",
        "a = #\n",
        "a = ##\n",
        "a = \"\n",
        "a = '\n",
        "a = x'\n",
        "a = 'x\n",
        "a = x\"y\n",
        "a = a'b'c\n",
        "a = \"x\"y\n",
        "a = x \"y\"\n",
        // Whitespace and separators.
        "  a = x\n",
        "\ta = x\n",
        "a=x\n",
        "a   =   x\n",
        "a = x  # c  \n",
        "a = ' , '\n",
        "a = \",\"\n",
        "a = x , y\n",
        "a = x ,y\n",
        "a = 'a', 'b', 'c'\n",
        "a = 'a' , \"b\"\n",
        // Section shapes.
        "[ s ]\na = x\n",
        "[s ]\n",
        "[ s]\n",
        "[s]\n[[t]]\n[[u]]\n",
        "[s]\n[[t]]\n[[[u]]]\n[[v]]\n",
        "[s]\n[[t]]\n[[[u]]]\n[w]\n",
        "[\"s\"]\n",
        "['s']\n",
        "[s#t]\n",
        "[s] # c\n",
        "[s]#c\n",
        // Triple quotes.
        "a = \"\"\"\"\"\"\n",
        "a = ''''''\n",
        "a = \"\"\"x'y\"\"\"\n",
        "a = '''x\"y'''\n",
        "a = \"\"\"a\nb\nc\"\"\"\n",
        "a = '''a\n'''\n",
        "a = \"\"\"has # hash\"\"\"\n",
        "a = \"\"\"has, comma\"\"\"\n",
        "[s]\na = \"\"\"x\ny\"\"\"\nb = z\n",
        // Comments in odd places.
        "#\n",
        "#c\na = x\n#d\nb = y\n#e\n",
        "[s]\n# inside\na = x\n",
        "a = x\n[s]\n# trailing in section\n",
        // Duplicates and errors.
        "a = 1\na = 2\n",
        "[s]\n[s]\n",
        "[s]\na = 1\na = 2\n",
        "not a valid line\n",
        "= x\n",
        "[unclosed\n",
        "[[a]]\n",
        "[s]\n[[[t]]]\n",
        // Nesting, re-entry and malformed headers.
        "[a]\n[[b]]\nx=1\n[[c]]\ny=2\n",
        "[a]\n[[b]]\n[[[c]]]\nx=1\n[[d]]\ny=2\n",
        "[[[a]]]\n",
        "[a]\n[[b]]\n[[[[c]]]]\n",
        "[a]\nx=1\n[a]\ny=2\n",
        "[]\n",
        "[ ]\n",
        "[a\n",
        "a]\n",
        // Unterminated multiline values.
        "a = '''x\n",
        "a = \"\"\"x\ny\n",
        "a = '''x\ny\"\"\"\n",
    ]
}

/// Mechanically generated inputs, to reach combinations the hand-written
/// corpus does not think of.
fn generated_corpus() -> Vec<String> {
    let fragments = [
        "x", "", " ", "'", "\"", "#", ",", "x,y", "'x'", "\"x\"", "x y", " x ", "'x", "x'", "a'b",
        "a\"b", "#x", "x#", ",x", "x,", "'x,y'", "=", "[x]", "'''x'''",
    ];
    let mut out = Vec::new();
    for fragment in fragments {
        out.push(format!("a = {fragment}\n"));
        out.push(format!("[s]\na = {fragment}\n"));
        out.push(format!("a = {fragment} # c\n"));
    }
    out
}

/// Escape a string as JSON, the way Python's json module would.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
