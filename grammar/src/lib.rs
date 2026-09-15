//! The grammar of tree-sitter-cpp, in Rust.
//!
//! `grammar()` gives the C++ grammar. It extends `c::grammar()`, the port of the
//! grammar of tree-sitter-c 0.24.1. `xtask generate` writes `src/grammar.json` from it.

pub mod c;
pub mod cpp;
pub mod dsl;

/// The C++ grammar.
pub fn grammar() -> dsl::Grammar {
    cpp::grammar()
}
