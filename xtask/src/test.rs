//! Run the corpus tests in `test/corpus` with the C++ grammar.
//!
//! The file format and the comparison are those of `tree-sitter test` 0.26. A test file
//! holds examples. Each example has a header with a name and optional attributes, the
//! source code, a divider line, and the expected syntax tree as an S-expression.
//!
//! `--update` writes a new expected tree only for the examples that it selects and that fail. The other
//! bytes of the file do not change, also the comments and the layout of the other expected trees.

use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use regex::bytes::{Regex as ByteRegex, RegexBuilder as ByteRegexBuilder};
use tree_sitter::{Language, Parser};

use crate::sexp;

/// A header: a line of three or more `=`, the name and attribute lines, and a second line of `=`.
static HEADER: LazyLock<ByteRegex> = LazyLock::new(|| {
    ByteRegexBuilder::new(
        r"^(?x)
           (?P<equals>(?:=+){3,})
           (?P<suffix1>[^=\r\n][^\r\n]*)?
           \r?\n
           (?P<name_and_attributes>(?:([^=\r\n]|\s+:)[^\r\n]*\r?\n)+)
           ===+
           (?P<suffix2>[^=\r\n][^\r\n]*)?\r?\n",
    )
    .multi_line(true)
    .build()
    .expect("the header expression is valid")
});

/// A divider: a line of three or more `-` between the source code and the expected tree.
static DIVIDER: LazyLock<ByteRegex> = LazyLock::new(|| {
    ByteRegexBuilder::new(r"^(?P<hyphens>(?:-+){3,})(?P<suffix>[^-\r\n][^\r\n]*)?\r?\n")
        .multi_line(true)
        .build()
        .expect("the divider expression is valid")
});

/// A comment line in an expected tree.
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*;.*$").expect("the expression is valid"));
/// A run of white space.
static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("the expression is valid"));
/// A field name in an S-expression.
static FIELD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" \w+: \(").expect("the expression is valid"));

/// The attributes of an example.
struct Attributes {
    /// `:skip`: do not run the example.
    skip: bool,
    /// `:error`: the tree must have an ERROR or MISSING node, and the expected tree is not compared.
    error: bool,
    /// `:fail-fast`: stop the run when the example fails.
    fail_fast: bool,
    /// `:platform(OS)`: false when all the platforms of the example differ from this operating system.
    platform: bool,
}

/// One example of a test file.
struct Example {
    name: String,
    attributes: Attributes,
    input: Vec<u8>,
    /// The expected S-expression, with no comments and with normalized white space.
    expected: String,
    /// True when the expected S-expression has field names. Otherwise the comparison ignores fields.
    has_fields: bool,
    /// The bytes of the file from the end of the divider line to the next header or to the end of the file.
    /// They hold the expected tree and the white space around it.
    output: Range<usize>,
}

/// A header of a test file.
struct Header {
    start: usize,
    end: usize,
    name: String,
    attributes: Attributes,
}

/// The result of one example.
enum Outcome {
    Passed,
    Skipped,
    /// `has_error` is true when the actual tree has an ERROR or a MISSING node.
    Failed {
        actual: String,
        has_error: bool,
    },
}

/// A failed example, for the report after the run.
struct Failure {
    label: String,
    expected: String,
    actual: String,
}

/// Read the name and the attributes from the lines of a header.
///
/// The name is the lines before the first attribute. `:cst` and `:language(...)` are errors,
/// because this runner has one language and compares S-expressions only.
fn parse_header(lines: &str) -> Result<(String, Attributes), String> {
    let mut name = String::new();
    let mut attributes = Attributes {
        skip: false,
        error: false,
        fail_fast: false,
        platform: true,
    };
    let mut platform: Option<bool> = None;
    let mut seen_attribute = false;
    for line in lines.split_inclusive('\n') {
        let trimmed = line.trim();
        match trimmed.split('(').next().unwrap_or_default() {
            ":skip" => (seen_attribute, attributes.skip) = (true, true),
            ":error" => (seen_attribute, attributes.error) = (true, true),
            ":fail-fast" => (seen_attribute, attributes.fail_fast) = (true, true),
            ":platform" => {
                if let Some(os) = trimmed.strip_prefix(":platform(").and_then(|s| s.strip_suffix(')')) {
                    seen_attribute = true;
                    platform = Some(platform.unwrap_or(false) || os.trim() == std::env::consts::OS);
                }
            }
            ":cst" | ":language" => return Err(format!("the attribute `{trimmed}` is not supported")),
            _ if !seen_attribute => name.push_str(line),
            _ => {}
        }
    }
    // `:skip` wins over `:error`, as in `tree-sitter test`.
    attributes.error &= !attributes.skip;
    attributes.platform = platform.unwrap_or(true);
    Ok((name.trim_end().to_owned(), attributes))
}

/// Read the examples of a test file, as `tree-sitter test` reads them.
///
/// The first header sets the suffix of the file, and headers and dividers with a different
/// suffix are text. The divider of an example is the longest divider between its header and
/// the next header. An example with no divider is not an example. O(n) in the file size.
fn parse_examples(content: &str) -> Result<Vec<Example>, String> {
    let bytes = content.as_bytes();
    let first_suffix = HEADER
        .captures(bytes)
        .and_then(|c| c.name("suffix1"))
        .map(|m| m.as_bytes());
    let mut headers = Vec::new();
    for captures in HEADER.captures_iter(bytes) {
        let suffix1 = captures.name("suffix1").map(|m| m.as_bytes());
        let suffix2 = captures.name("suffix2").map(|m| m.as_bytes());
        if suffix1 != first_suffix || suffix2 != first_suffix {
            continue;
        }
        let whole = captures.get(0).expect("a match has a whole range");
        let lines = str::from_utf8(&captures["name_and_attributes"]).map_err(|e| e.to_string())?;
        let (name, attributes) = parse_header(lines)?;
        headers.push(Header {
            start: whole.start(),
            end: whole.end(),
            name,
            attributes,
        });
    }
    let ends: Vec<usize> = headers.iter().skip(1).map(|h| h.start).chain([bytes.len()]).collect();
    let mut examples = Vec::with_capacity(headers.len());
    for (header, end) in headers.into_iter().zip(ends) {
        let body = &bytes[header.end..end];
        let divider = DIVIDER
            .captures_iter(body)
            .filter(|c| c.name("suffix").map(|m| m.as_bytes()) == first_suffix)
            .max_by_key(|c| c.get(0).map_or(0, |m| m.len()));
        let Some(divider) = divider else { continue };
        let whole = divider.get(0).expect("a match has a whole range");
        let Ok(output) = str::from_utf8(&body[whole.end()..]) else {
            continue;
        };
        let mut input = body[..whole.start()].to_vec();
        input.pop();
        if input.last() == Some(&b'\r') {
            input.pop();
        }
        let output = COMMENT.replace_all(output, "");
        let expected = WHITESPACE.replace_all(output.trim(), " ").replace(" )", ")");
        examples.push(Example {
            has_fields: FIELD.is_match(&expected),
            expected,
            input,
            output: header.end + whole.end()..end,
            name: header.name,
            attributes: header.attributes,
        });
    }
    Ok(examples)
}

/// Parse an example and compare its tree with the expected tree.
fn check(parser: &mut Parser, example: &Example) -> Outcome {
    if example.attributes.skip || !example.attributes.platform {
        return Outcome::Skipped;
    }
    let tree = parser
        .parse(&example.input, None)
        .expect("a parser with a language and no time limit gives a tree");
    let root = tree.root_node();
    let has_error = root.has_error();
    if example.attributes.error {
        return if has_error {
            Outcome::Passed
        } else {
            Outcome::Failed {
                actual: root.to_sexp(),
                has_error,
            }
        };
    }
    let mut actual = root.to_sexp();
    if !example.has_fields {
        actual = FIELD.replace_all(&actual, " (").into_owned();
    }
    if actual == example.expected {
        Outcome::Passed
    } else {
        Outcome::Failed { actual, has_error }
    }
}

/// The text of a test file with new expected trees.
///
/// Each replacement gives the `output` range of an example and the formatted tree for it. The replacements
/// are in file order. The new tree takes the place of the old tree, and the white space around the old tree
/// stays. When the range holds only white space, the range gets a blank line, the tree, and a line break, and a
/// blank line before the next header. The other bytes of the file do not change. O(n) in the length of the file.
fn replace_outputs(content: &str, replacements: &[(Range<usize>, String)]) -> String {
    let mut text = String::with_capacity(content.len());
    let mut copied = 0;
    for (range, tree) in replacements {
        let old = &content[range.clone()];
        let trimmed = old.trim();
        if trimmed.is_empty() {
            text.push_str(&content[copied..range.start]);
            text.push('\n');
            text.push_str(tree);
            text.push('\n');
            if range.end < content.len() {
                text.push('\n');
            }
        } else {
            let start = range.start + (old.len() - old.trim_start().len());
            text.push_str(&content[copied..start]);
            text.push_str(tree);
            text.push_str(&old[old.trim_end().len()..]);
        }
        copied = range.end;
    }
    text.push_str(&content[copied..]);
    text
}

/// The paths of the test files under a directory, sorted by name in each directory, without hidden files.
fn test_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let mut entries: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|e| format!("cannot read {}: {e}", directory.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    entries.retain(|path| {
        !path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'))
    });
    entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    for path in entries {
        if path.is_dir() {
            test_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

/// The name and the bytes of an input.
pub(crate) type NamedInput = (String, Vec<u8>);

/// The name and the input of each example of the test files in `test/corpus`, in the order of the test run.
/// The name is the path of the file under `test/corpus`, `#`, and the name of the example.
pub(crate) fn example_inputs(repository: &Path) -> Result<Vec<NamedInput>, Box<dyn Error>> {
    let corpus = repository.join("test").join("corpus");
    let mut files = Vec::new();
    test_files(&corpus, &mut files)?;
    let mut inputs = Vec::new();
    for file in &files {
        let content = fs::read_to_string(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        let relative = file.strip_prefix(&corpus).unwrap_or(file).display().to_string();
        for example in parse_examples(&content).map_err(|e| format!("{}: {e}", file.display()))? {
            inputs.push((format!("{relative}#{}", example.name), example.input));
        }
    }
    Ok(inputs)
}

/// Print the lines between the common first lines and the common last lines of two texts.
fn print_difference(expected: &str, actual: &str) {
    let expected: Vec<&str> = expected.lines().collect();
    let actual: Vec<&str> = actual.lines().collect();
    let prefix = expected.iter().zip(&actual).take_while(|(e, a)| e == a).count();
    let suffix = expected[prefix..]
        .iter()
        .rev()
        .zip(actual[prefix..].iter().rev())
        .take_while(|(e, a)| e == a)
        .count();
    for line in &expected[prefix.saturating_sub(3)..prefix] {
        println!("    {line}");
    }
    for line in &expected[prefix..expected.len() - suffix] {
        println!("  - {line}");
    }
    for line in &actual[prefix..actual.len() - suffix] {
        println!("  + {line}");
    }
    for line in expected[expected.len() - suffix..].iter().take(3) {
        println!("    {line}");
    }
}

/// Run the corpus tests. With `--update`, write the actual tree of each failed example that has
/// no error and no `:error` attribute into its test file. With a NAME, run only the examples whose
/// name contains it.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut update = false;
    let mut filter: Option<&str> = None;
    for arg in args {
        match arg.as_str() {
            "--update" => update = true,
            text if !text.starts_with('-') && filter.is_none() => filter = Some(text),
            _ => return Err("usage: cargo xtask test [--update] [NAME]".into()),
        }
    }
    let corpus = repository.join("test").join("corpus");
    let mut files = Vec::new();
    test_files(&corpus, &mut files)?;
    let mut parser = Parser::new();
    parser.set_language(&Language::new(tree_sitter_cpp::LANGUAGE))?;

    let (mut passed, mut skipped) = (0usize, 0usize);
    let mut failures = Vec::new();
    let mut stop = false;
    for file in &files {
        let content = fs::read_to_string(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        let examples = parse_examples(&content).map_err(|e| format!("{}: {e}", file.display()))?;
        let group = file
            .strip_prefix(&corpus)
            .unwrap_or(file)
            .with_extension("")
            .display()
            .to_string();
        println!("{group}:");
        let mut replacements = Vec::new();
        for example in &examples {
            let selected = filter.is_none_or(|name| example.name.contains(name));
            let outcome = if selected {
                check(&mut parser, example)
            } else {
                Outcome::Skipped
            };
            match outcome {
                Outcome::Passed => {
                    passed += 1;
                    println!("  ✓ {}", example.name);
                }
                Outcome::Skipped => {
                    if selected {
                        skipped += 1;
                        println!("  - {}", example.name);
                    }
                }
                Outcome::Failed { actual, has_error } => {
                    println!("  ✗ {}", example.name);
                    let actual = sexp::format(&actual);
                    if update && !example.attributes.error && !has_error {
                        replacements.push((example.output.clone(), actual.clone()));
                    }
                    failures.push(Failure {
                        label: format!("{group}: {}", example.name),
                        expected: if example.attributes.error {
                            "an ERROR or a MISSING node".to_owned()
                        } else {
                            sexp::format(&example.expected)
                        },
                        actual,
                    });
                    if example.attributes.fail_fast {
                        stop = true;
                        break;
                    }
                }
            }
        }
        if !replacements.is_empty() {
            let text = replace_outputs(&content, &replacements);
            fs::write(file, text).map_err(|e| format!("cannot write {}: {e}", file.display()))?;
        }
        if stop {
            break;
        }
    }

    for (index, failure) in failures.iter().enumerate() {
        println!("\n{}. {}", index + 1, failure.label);
        print_difference(&failure.expected, &failure.actual);
    }
    println!("\n{passed} passed, {} failed, {skipped} skipped", failures.len());
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("{} corpus tests failed", failures.len()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_example_has_a_name_attributes_an_input_and_a_normalized_tree() {
        let content = "====\nfirst\n:skip\n====\nint x;\n---\n\n(translation_unit\n  ; a comment\n  (declaration) )\n";
        let examples = parse_examples(content).expect("the content is valid");
        assert_eq!(examples.len(), 1);
        let example = &examples[0];
        assert_eq!(example.name, "first");
        assert!(example.attributes.skip);
        assert_eq!(example.input, b"int x;");
        assert_eq!(example.expected, "(translation_unit (declaration))");
        assert!(!example.has_fields);
        assert_eq!(
            &content[example.output.clone()],
            "\n(translation_unit\n  ; a comment\n  (declaration) )\n"
        );
    }

    #[test]
    fn the_longest_divider_separates_the_input_from_the_tree() {
        let content = "===\nt\n===\na\n---\nb\n-----\n\n(x field: (y))\n";
        let example = &parse_examples(content).expect("the content is valid")[0];
        assert_eq!(example.input, b"a\n---\nb");
        assert!(example.has_fields);
    }

    /// The update of one example keeps the comments, the MISSING nodes, and the layout of the other examples.
    /// Before, the update wrote each example of the file again with `tree_sitter::format_sexp`, and it moved
    /// the last of three MISSING nodes of a different example after its root.
    #[test]
    fn an_update_changes_only_the_expected_tree_of_the_selected_example() {
        let first = "===\nkeep\n:error\n===\nint\n---\n\n(ERROR)  ; a comment\n\n";
        let second = "======\nchange\n======\nx;\n------\n\n(translation_unit\n  (old))\n\n";
        let third = "===\nmissing\n===\n{{{\n---\n\n(translation_unit\n  (a\n    (b\n      (MISSING \"}\"))\n    \
                     (MISSING \"}\"))\n  (MISSING \"}\"))\n";
        let content = format!("{first}{second}{third}");
        let examples = parse_examples(&content).expect("the content is valid");
        assert_eq!(examples.len(), 3);
        let tree = sexp::format("(translation_unit (expression_statement (identifier)))");
        let updated = replace_outputs(&content, &[(examples[1].output.clone(), tree.clone())]);
        let expected_second = format!("======\nchange\n======\nx;\n------\n\n{tree}\n\n");
        assert_eq!(updated, format!("{first}{expected_second}{third}"));
        let again = parse_examples(&updated).expect("the updated content is valid");
        assert_eq!(
            again[1].expected,
            "(translation_unit (expression_statement (identifier)))"
        );
        assert_eq!(again[2].expected, examples[2].expected);
    }

    #[test]
    fn an_update_of_an_empty_tree_adds_a_blank_line_and_the_tree() {
        let content = "===\nempty\n===\nx;\n---\n===\nlast\n===\ny;\n---\n";
        let examples = parse_examples(content).expect("the content is valid");
        assert_eq!(examples.len(), 2);
        let replacements = [
            (examples[0].output.clone(), "(a)".to_owned()),
            (examples[1].output.clone(), "(b)".to_owned()),
        ];
        let updated = replace_outputs(content, &replacements);
        assert_eq!(
            updated,
            "===\nempty\n===\nx;\n---\n\n(a)\n\n===\nlast\n===\ny;\n---\n\n(b)\n"
        );
        let again = parse_examples(&updated).expect("the updated content is valid");
        assert_eq!((again[0].expected.as_str(), again[1].expected.as_str()), ("(a)", "(b)"));
    }

    /// The layout of each expected tree in `test/corpus` is the layout of `sexp::format`. An update of an
    /// example then gives the layout of the other examples.
    #[test]
    fn the_expected_trees_of_the_corpus_have_the_layout_of_the_formatter() {
        let mut files = Vec::new();
        test_files(&crate::repository().join("test").join("corpus"), &mut files).expect("test/corpus is readable");
        for file in files {
            let content = fs::read_to_string(&file).expect("a test file is readable");
            for example in parse_examples(&content).expect("a test file is valid") {
                let text = content[example.output.clone()].trim();
                if !COMMENT.is_match(text) {
                    assert_eq!(
                        text,
                        sexp::format(&example.expected),
                        "{}: {}",
                        file.display(),
                        example.name
                    );
                }
            }
        }
    }
}
