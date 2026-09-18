# configobj-rs

A Rust reader and writer for the [ConfigObj](https://github.com/DiffSK/configobj)
configuration file format.

The format is INI-like, with a few additions: sections nest by repeating the
brackets, values can be comma-separated lists, and comments are attached to the
entries they precede, so rewriting a file leaves it looking the way its author
wrote it.

```ini
# who to blame
name = Jelmer
languages = rust, python

[editor]
command = vi

    [[options]]
    tabs = no
```

## Usage

```rust
use configobj::ConfigObj;

let mut conf = ConfigObj::from_file("bazaar.conf")?;

assert_eq!(conf.get_str("name"), Some("Jelmer"));
assert_eq!(conf.get_list("languages"), Some(vec!["rust", "python"]));
assert_eq!(
    conf.section("editor").and_then(|s| s.get_str("command")),
    Some("vi"),
);

conf.entry_section("editor").insert("command", "emacs");
conf.save()?;
```

Values are text until you ask for something else, which mirrors what the file
format actually stores:

```rust
let debug: bool = conf.get_bool("debug").transpose()?.unwrap_or(false);
let port: u16 = conf.get("port").map(|v| v.parse()).transpose()?.unwrap_or(8080);
```

A value that is not a valid boolean is an error rather than a silent `false`,
so a typo in a config file gets reported instead of quietly changing behaviour.

`Builder` covers the reading options:

```rust
use configobj::Builder;

// Keep values exactly as written, without splitting lists or unquoting.
let conf = Builder::new().list_values(false).from_str(text)?;
```

## Compatibility

The file format is compatible with the Python implementation, and the test
suite checks this: `tests/compat.rs` compares parsing against expectations
recorded from configobj 5.0.9 over a corpus of 195 inputs in both list modes.
Those expectations live in `tests/data/reference.txt` and are committed, so
the tests need no Python installed and cannot be perturbed by whatever happens
to be on the machine. `tests/README.md` covers regenerating them.

The API is not a transliteration of the Python one. Notably:

- A `Section` is an ordered map rather than a `dict` subclass, and lookups
  return `Option` instead of raising `KeyError`.
- Scalars and subsections are distinguished at the type level, so
  `get` never hands back a section.
- A single value and a one-element list stay distinct, matching the file
  format (`a = x` against `a = x,`).
- Comments are attached to entries through `comments()` rather than through
  parallel dictionaries.

Three places deliberately differ from the Python implementation's behaviour,
because it writes files it cannot read back:

- Values containing `'''` or `"""` are quoted with a triple quote that does not
  collide with the value ([lp:710410], which breezy patches around).
- Keys containing `=` are quoted, where Python writes them bare and then reads
  back a truncated key.
- With list values off, a value that could not be read back as itself is
  refused rather than written. Python writes `a = a#b` there and reads back
  `a`, losing the rest without saying so.

Interpolation (`%(name)s` and `$name`) is not implemented. Breezy disables it
and does its own option expansion, and leaving it out means a value is always
exactly what the file says.

Validation against a configspec is also not implemented.

[lp:710410]: https://bugs.launchpad.net/bzr/+bug/710410

## Testing

```console
$ cargo test
```

The suite covers format details, round-tripping, the behaviour breezy relies
on, a differential comparison against Python, and a fuzz pass that checks
awkward input produces errors rather than panics.

## License

BSD-3-Clause, matching the Python implementation.
