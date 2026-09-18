//! Print the compatibility corpus, one JSON-escaped input per line.
//!
//! Used to regenerate tests/data/reference.txt; see that file's header.
include!("../tests/shared/corpus.rs");

fn main() {
    for text in corpus() {
        println!("{}", json_string(&text));
    }
}
