//! The development tasks of tree-sitter-cpp.
//!
//! Run a task with `cargo xtask TASK`. The alias is in `.cargo/config.toml`.

mod corpus;
mod differ;
mod directives;
mod fuzz;
mod generate;
mod limits;
mod parse;
mod precedence;
mod sexp;
mod syntax;
mod test;
mod ties;
mod trees;
mod vendor;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask TASK

tasks:
  generate [--check]       Write src/grammar.json from the Rust grammar. Then write src/parser.c,
                           src/node-types.json, and src/tree_sitter/ from src/grammar.json.
                           With --check, compare src/grammar.json with the Rust grammar only.
                           With --diagnostics, print each diagnostic of the generator.
                           Fail when the grammar declares a conflict set that the parse table
                           builder does not use.
  limits [PARSER_C]        Print the use of each fixed-width limit of the parser tables of
                           src/parser.c or PARSER_C. Fail when a use is more than 90% of its limit.
  precedence [PARSER_C]    Print each dynamic precedence of src/grammar.json and the values of the
                           reduce actions of src/parser.c or PARSER_C. Fail when the parse table
                           holds no reduce action with a value that the grammar declares.
  ties table               Print each action list of src/parser.c that holds more than one action,
                           by class and by the rules that the reduce actions reduce.
  ties corpus [ROOT LIST]  Parse each file of LIST with the logger of the runtime, and print each
                           site where the order of the symbol ids selects the tree. Fail for a site
                           that test/ties/baseline.txt does not hold. With --write-baseline before
                           ROOT, write the sites to that file and fail for none. With no ROOT and no
                           LIST, read /tmp/cpp-corpora and test/ties/sample.txt.
  ties versions [ROOT LIST]
                           Print each file of LIST whose GLR version count passes MAX_VERSION_COUNT
                           of the runtime. The condense step then removes a version by arrival
                           order. Fail for a file that test/ties/versions.txt does not hold.
  ties trace FILE          Print the parse log of FILE. With --state N, print only the rows where
                           the parser enters the state N.
  test [--update] [NAME]   Run the corpus tests in test/corpus. With --update, write the actual
                           tree of each failed test that has no error into the test file.
  syntax [NAME]            Parse each snippet in test/syntax, and report each parse error.
                           With a NAME, also print the tree of each snippet that it selects.
  parse FILE               Print the syntax tree of FILE. With -, read the standard input.
  corpus ROOT LIST OUT     Parse each file of LIST, a list of paths relative to ROOT. Write the
                           parse errors of each file to OUT, one TSV line for each file.
  directives ROOT LIST BASELINE
                           Check that no node outside a directive begins on a line whose first
                           character that is not a blank is `#`. Such a node holds a token that the
                           preprocessor discards. Fail on a site that BASELINE does not hold, and
                           report a site of BASELINE that is gone without a failure.
  trees ROOT LIST OUT      Parse each file of LIST, and write one TSV line for each file to OUT:
                           the path, 1 if the tree has an error, a hash of the full tree that is
                           the same in each build, the node count, the byte and the kind of the
                           first error, and the hashes of the blocks of the file.
  trees --compare A B      Compare two outputs of `trees`. Print the count of files, of files
                           whose hash changed with no error in A, and of files whose hash changed
                           with an error in A. Then print the first 60 paths of the first group,
                           with the byte range of the first node that differs.
  differ ROOT LIST OUT     Compare the trees of the files of LIST with GCC and Clang, and write the
                           disagreements to the directory OUT. `cargo xtask differ --help` gives
                           the modes for the syntax snippets and for one snippet.
  fuzz OUT                 Parse small changes of the syntax snippets, of the inputs of the corpus
                           tests, and of the files of an optional list in child processes. Write each
                           changed input whose parse does not stop in a time limit, stops the process,
                           or gives an incorrect range to the directory OUT. `cargo xtask fuzz --help`
                           gives the options.
  vendor --to VERSION      Put the repairs of the fork on the release VERSION of vendor/tree-sitter
                           and vendor/tree-sitter-generate. The merge uses the pristine sources in
                           vendor/upstream as the base, and it writes the conflict markers of
                           `git merge-file` into each file with a conflict.
  vendor --check           Compare the pristine sources in vendor/upstream with the hashes of the
                           last update.";

/// The directory of the repository: the parent directory of the xtask crate.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the xtask crate is in a directory of the repository")
        .to_owned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result: Result<(), Box<dyn Error>> = match args.first().map(String::as_str) {
        Some("generate") => generate::run(&repository(), &args[1..]),
        Some("limits") => limits::run(&repository(), &args[1..]),
        Some("precedence") => precedence::run(&repository(), &args[1..]),
        Some("test") => test::run(&repository(), &args[1..]),
        Some("ties") => ties::run(&repository(), &args[1..]),
        Some("syntax") => syntax::run(&repository(), &args[1..]),
        Some("parse") => parse::run(&args[1..]),
        Some("corpus") => corpus::run(&args[1..]),
        Some("directives") => directives::run(&args[1..]),
        Some("trees") => trees::run(&args[1..]),
        Some("differ") => differ::run(&repository(), &args[1..]),
        Some("fuzz") => fuzz::run(&repository(), &args[1..]),
        Some("vendor") => vendor::run(&repository(), &args[1..]),
        // The child process of `fuzz`. The usage does not show it.
        Some("fuzz-worker") => fuzz::worker(),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
