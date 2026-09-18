"""The behaviour breezy expects of configobj, checked against the binding.

Run with the built extension importable, e.g.:

    cargo build -p configobj-py --release
    cp target/release/lib_configobj_rs.so /tmp/x/_configobj_rs.so
    PYTHONPATH=/tmp/x python3 configobj-py/tests/test_binding.py
"""

import unittest
from io import BytesIO

import _configobj_rs as configobj
from _configobj_rs import ConfigObj

bool_config = b"""[DEFAULT]
active = true
inactive = false
[UPPERCASE]
active = True
nonactive = False
"""

erroneous_config = b"""[section] # line 1
good=good # line 2
[section] # line 3
whocares=notme # line 4
"""

class T(unittest.TestCase):
    def test_get_bool(self):
        co = ConfigObj(BytesIO(bool_config))
        self.assertTrue(co["DEFAULT"].as_bool("active"))
        self.assertFalse(co["DEFAULT"].as_bool("inactive"))
        self.assertTrue(co["UPPERCASE"].as_bool("active"))
        self.assertFalse(co["UPPERCASE"].as_bool("nonactive"))

    def test_hash_sign_in_value(self):
        co = ConfigObj()
        co["test"] = "foo#bar"
        outfile = BytesIO()
        co.write(outfile=outfile)
        lines = outfile.getvalue().splitlines()
        self.assertEqual(lines, [b'test = "foo#bar"'])
        co2 = ConfigObj(lines)
        self.assertEqual(co2["test"], "foo#bar")

    def test_triple_quotes(self):
        triple_quotes_value = '''spam
""" that's my spam """
eggs'''
        co = ConfigObj()
        co["test"] = triple_quotes_value
        outfile = BytesIO()
        co.write(outfile=outfile)
        output = outfile.getvalue()
        co2 = ConfigObj(BytesIO(output))
        self.assertEqual(triple_quotes_value, co2["test"])

    def test_duplicate_section_name_error_line(self):
        try:
            ConfigObj(BytesIO(erroneous_config), raise_errors=True)
        except configobj.DuplicateError as e:
            self.assertEqual(3, e.line_number)
        else:
            self.fail("Error in config file not detected")

    def test_setdefault_creates_section(self):
        co = ConfigObj()
        co.setdefault("sect", {})["opt"] = "v"
        self.assertEqual(co["sect"]["opt"], "v")

    def test_delitem(self):
        co = ConfigObj([b"a = 1", b"b = 2"])
        del co["a"]
        self.assertNotIn("a", co)
        self.assertIn("b", co)

    def test_contains_and_get(self):
        co = ConfigObj([b"a = 1"])
        self.assertIn("a", co)
        self.assertEqual(co.get("a"), "1")
        self.assertEqual(co.get("zz", "fallback"), "fallback")

    def test_bytes_lines_input(self):
        # breezy/bzr/remote.py passes a list of bytes lines.
        co = ConfigObj([b"[DEFAULT]", b"email = Jelmer"])
        self.assertEqual(co["DEFAULT"]["email"], "Jelmer")

    def test_list_values_false(self):
        co = ConfigObj([b'a = x, y'], list_values=False)
        self.assertEqual(co["a"], "x, y")

    def test_as_int(self):
        co = ConfigObj([b"port = 8080"])
        self.assertEqual(co.as_int("port"), 8080)

    def test_write_returns_lines_without_outfile(self):
        co = ConfigObj()
        co["a"] = "x"
        self.assertEqual(co.write(), ["a = x"])

    def test_held_section_sees_later_writes(self):
        # breezy holds a section and mutates the config through it.
        co = ConfigObj([b"[s]", b"a = 1"])
        sec = co["s"]
        co["s"]["b"] = "2"
        self.assertEqual(sec["b"], "2")
        sec["c"] = "3"
        self.assertEqual(co["s"]["c"], "3")

    def test_setdefault_keeps_an_existing_section(self):
        co = ConfigObj([b"[s]", b"a = 1"])
        co.setdefault("s", {})["b"] = "2"
        self.assertEqual(co["s"].keys(), ["a", "b"])

    def test_quote_unquote_round_trip(self):
        # breezy's config stores quote values themselves; these forms come
        # from its own test suite.
        for value in ['" a b c "', '" a , b c "', '","', "plain", "with space"]:
            self.assertEqual(configobj.unquote(configobj.quote(value)), value)

    def test_parse_value_splits_lists(self):
        # Replaces breezy's reset()/_parse() dance for its list converter.
        self.assertEqual(configobj.parse_value("a, b"), ["a", "b"])
        self.assertEqual(configobj.parse_value("a,"), ["a"])
        self.assertEqual(configobj.parse_value(","), [])
        self.assertEqual(configobj.parse_value("a"), "a")
        self.assertEqual(configobj.parse_value("'a, b'"), "a, b")

    def test_comments_survive_a_rewrite(self):
        data = b"# header\na = 1\n# about b\nb = 2\n"
        co = ConfigObj(BytesIO(data))
        out = BytesIO()
        co.write(outfile=out)
        self.assertEqual(out.getvalue(), data)

    def test_missing_key_raises_key_error(self):
        co = ConfigObj([b"a = 1"])
        self.assertRaises(KeyError, lambda: co["nope"])

    def test_parse_error_carries_line_number(self):
        try:
            ConfigObj([b"[s]", b"not a config line"])
        except configobj.ParseError as e:
            self.assertEqual(2, e.line_number)
        else:
            self.fail("bad line not detected")


    def test_subclassing_as_breezy_does(self):
        # breezy/config.py subclasses ConfigObj, forwarding the input and
        # extra keywords to super().__init__.
        class Sub(ConfigObj):
            def __init__(self, infile=None, **kwargs):
                super().__init__(infile, interpolation=False, **kwargs)

            def get_bool(self, section, key):
                return self[section].as_bool(key)

        co = Sub(BytesIO(b"[DEFAULT]\nactive = true\n"))
        self.assertTrue(co.get_bool("DEFAULT", "active"))
        co["DEFAULT"]["x"] = "1"
        out = BytesIO()
        co.write(outfile=out)
        self.assertEqual(out.getvalue(), b"[DEFAULT]\nactive = true\nx = 1\n")


    def test_iteritems(self):
        # breezy still uses the py2-era name.
        co = ConfigObj([b"a = 1", b"b = 2"])
        self.assertEqual(co.iteritems(), [("a", "1"), ("b", "2")])

    def test_section_compares_equal_to_a_dict(self):
        # The Python configobj's Section subclasses dict, so callers compare
        # sections against plain dicts and format them as dicts.
        co = ConfigObj([b"[s]", b"a = 1"])
        self.assertEqual({"a": "1"}, co["s"])
        self.assertEqual(co["s"], {"a": "1"})
        self.assertEqual(f"{co['s']}", "{'a': '1'}")

    def test_configs_compare_equal_by_content(self):
        self.assertEqual(ConfigObj([b"a = 1"]), ConfigObj([b"a = 1"]))
        self.assertNotEqual(ConfigObj([b"a = 1"]), ConfigObj([b"a = 2"]))

    def test_missing_file_reads_as_empty(self):
        # An optional config file that is not there is not an error.
        co = ConfigObj("/nonexistent/does-not-exist.conf")
        self.assertEqual(co.keys(), [])
        self.assertRaises(
            OSError, ConfigObj, "/nonexistent/does-not-exist.conf", file_error=True
        )

    def test_bad_encoding_raises_unicode_decode_error(self):
        # breezy catches UnicodeDecodeError to report a mis-encoded file
        # separately from a malformed one.
        self.assertRaises(UnicodeDecodeError, ConfigObj, BytesIO(b"a = \xff\n"))

    def test_parse_error_carries_errors_list(self):
        try:
            ConfigObj([b"[s]", b"not a config line"])
        except configobj.ParseError as e:
            self.assertEqual(1, len(e.errors))
            self.assertIn("line 2", e.errors[0].msg)
            self.assertIsNone(e.config.filename)
        else:
            self.fail("bad line not detected")


    def test_non_string_values_are_stored_as_text(self):
        # A config file holds only text, and the Python configobj stringifies
        # on write, so breezy stores bools and numbers directly.
        co = ConfigObj()
        co["n"] = 42
        co["b"] = True
        co["list"] = [1, "a", "with, a comma"]
        self.assertEqual(co["n"], "42")
        self.assertEqual(co["b"], "True")
        self.assertEqual(co["list"], ["1", "a", "with, a comma"])
        out = BytesIO()
        co.write(outfile=out)
        self.assertEqual(
            out.getvalue(),
            b'n = 42\nb = True\nlist = 1, a, "with, a comma"\n',
        )

    def test_quote_handles_any_value(self):
        # breezy quotes whatever a caller set, including bools and lists.
        self.assertEqual(configobj.quote(True), "True")
        self.assertEqual(configobj.quote(42), "42")
        self.assertEqual(configobj.quote(3.5), "3.5")
        self.assertEqual(configobj.quote(["a", "b"]), "a, b")
        self.assertEqual(configobj.quote(["only"]), "only,")
        self.assertEqual(configobj.quote([]), ",")


    def test_a_section_is_a_mapping(self):
        # The Python configobj's Section subclasses dict, and callers test
        # values with isinstance to tell a mapping from a string. A pyclass
        # cannot subclass dict, so Section registers with the ABCs instead.
        from collections.abc import Mapping, MutableMapping

        co = ConfigObj([b"[s]", b"a = 1"])
        self.assertIsInstance(co["s"], Mapping)
        self.assertIsInstance(co["s"], MutableMapping)

    def test_a_dict_value_is_stored_as_a_section(self):
        # breezy stores a dict as an option value and reads it back.
        co = ConfigObj()
        value = {"ascii": "abcd", "unicode": "foo"}
        co["name"] = value
        self.assertEqual(co["name"], value)
        out = BytesIO()
        co.write(outfile=out)
        self.assertEqual(out.getvalue(), b"[name]\nascii = abcd\nunicode = foo\n")


if __name__ == "__main__":
    unittest.main(verbosity=2)
