# Tests

- `format.rs` covers the details of the file format.
- `roundtrip.rs` checks that writing and re-reading a config preserves it.
- `breezy.rs` covers the behaviour breezy relies on, including cases taken
  from its own test suite.
- `compat.rs` checks parsing against expectations recorded from the Python
  implementation.
- `fuzz.rs` checks that awkward input produces errors rather than panics, and
  that anything written can be read back.

## Regenerating the compatibility reference

`data/reference.txt` records what the Python configobj makes of each input in
the shared corpus (`corpus.rs`). It is committed, so the tests need nothing
installed and do not depend on what happens to be on the machine.

It only needs regenerating when the corpus gains entries: `compat.rs` fails
with a list of the inputs that have no recorded answer. To regenerate, with a
Python that has `configobj` importable:

```console
$ cargo run --example dump_corpus | python3 - > /tmp/body <<'EOF'
import json, sys
from configobj import ConfigObj, Section, ConfigObjError

def convert(section):
    out = []
    for key in section.keys():
        value = section[key]
        if isinstance(value, Section):
            out.append([key, "section", convert(value)])
        elif isinstance(value, list):
            out.append([key, "list", list(value)])
        else:
            out.append([key, "string", value])
    return out

def describe(text, list_values):
    try:
        conf = ConfigObj(text.splitlines(), list_values=list_values,
                         interpolation=False)
    except ConfigObjError:
        return "ERROR"
    return convert(conf)

for line in sys.stdin:
    text = json.loads(line)
    for lv in (True, False):
        print("%s\t%d\t%s" % (json.dumps(text), 1 if lv else 0,
                              json.dumps(describe(text, lv))))
EOF
```

Then put the comment header from the top of the existing `data/reference.txt`
back on the front of `/tmp/body`, updating the recorded version if it changed,
and move it into place.

Where this crate deliberately differs from the Python implementation, the
reference is not the authority: those cases are listed in the README and
covered by their own tests.
