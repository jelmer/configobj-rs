//! Awkward input should produce an error, never a panic or a silent mangling.

use configobj::{Builder, ConfigObj};

/// Fragments chosen to land on the parser's decision points, including
/// multi-byte characters that byte-wise slicing would split.
const PIECES: &[&str] = &[
    "\u{133}",
    "\u{4e00}",
    "\u{1f600}",
    "a",
    "=",
    "[",
    "]",
    "'",
    "\"",
    "#",
    ",",
    " ",
    "\t",
    "'''",
    "\"\"\"",
    "\n",
];

#[test]
fn awkward_input_never_panics() {
    let mut checked = 0usize;
    for a in PIECES {
        for b in PIECES {
            for c in PIECES {
                for d in PIECES {
                    let text = format!("{a}{b}{c}{d}");
                    for list_values in [true, false] {
                        if let Ok(conf) = Builder::new().list_values(list_values).from_str(&text) {
                            let _ = conf.to_string();
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, PIECES.len().pow(4) * 2);
}

#[test]
fn anything_that_parses_and_writes_can_be_read_back() {
    // Whatever the writer emits has to be something the parser accepts, or a
    // config could be saved and then fail to load.
    let mut checked = 0usize;
    for a in PIECES {
        for b in PIECES {
            for c in PIECES {
                let text = format!("k = {a}{b}{c}");
                let Ok(conf) = ConfigObj::from_str(&text) else {
                    continue;
                };
                let Ok(written) = conf.to_string() else {
                    continue;
                };
                let back = ConfigObj::from_str(&written).unwrap_or_else(|e| {
                    panic!("parsed {text:?}, wrote {written:?}, could not re-read it: {e}")
                });
                assert_eq!(
                    back.get("k"),
                    conf.get("k"),
                    "value changed when {text:?} was written as {written:?}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "the corpus should produce parsable input");
}

#[test]
fn a_value_is_either_refused_or_survives_a_write() {
    // The writer must never emit something that reads back as a different
    // value: either it quotes the value so it survives, or it refuses.
    let mut checked = 0usize;
    for a in PIECES {
        for b in PIECES {
            for c in PIECES {
                let value = format!("{a}{b}{c}");
                for list_values in [true, false] {
                    let mut conf = Builder::new()
                        .list_values(list_values)
                        .from_str("")
                        .expect("empty input parses");
                    conf.insert("k", value.as_str());
                    let Ok(written) = conf.to_string() else {
                        continue;
                    };
                    let back = Builder::new()
                        .list_values(list_values)
                        .from_str(&written)
                        .unwrap_or_else(|e| {
                            panic!("wrote {written:?} for {value:?}, could not read it: {e}")
                        });
                    assert_eq!(
                        back.get_str("k"),
                        Some(value.as_str()),
                        "{value:?} (list_values={list_values}) came back changed from {written:?}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 0, "some values should be writable");
}
