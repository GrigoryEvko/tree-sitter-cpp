//! Write the TSV files and the summary of a run, and print the side-by-side view of one snippet.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Duration;

use super::compare::Verdict;
use super::label::Category;
use super::oracle::{JsonStatus, Oracle, Status};
use super::{Analysis, FileResult, excerpt, line_starts, position};

/// The column of a compiler status in the validity matrix.
fn column(status: Status) -> usize {
    match status {
        Status::Accept => 0,
        Status::Reject => 1,
        _ => 2,
    }
}

/// A percentage with one decimal.
fn percent(part: u64, whole: u64) -> String {
    if whole == 0 {
        "-".to_owned()
    } else {
        format!("{:.1}", 100.0 * part as f64 / whole as f64)
    }
}

/// A TSV field with no tabs and no line ends.
fn field(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

/// Write `validity.tsv` and `disagreements.tsv` to the directory `out`.
fn write_tables(out: &Path, results: &[FileResult]) -> Result<(), Box<dyn Error>> {
    let path = out.join("validity.tsv");
    let mut sink =
        BufWriter::new(fs::File::create(&path).map_err(|e| format!("cannot create {}: {e}", path.display()))?);
    writeln!(
        sink,
        "file\tlanguage\tbytes\tgcc\tgcc_error\tclang\tclang_error\tour_errors\tour_first_error\tjson\tjson_bytes\tgcc_flags\tclang_flags"
    )?;
    for r in results {
        writeln!(
            sink,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            field(&r.label),
            r.language.name(),
            r.bytes,
            r.gcc.status.name(),
            field(&r.gcc.error),
            r.clang.status.name(),
            field(&r.clang.error),
            if r.stopped {
                "stopped".to_owned()
            } else {
                r.our_errors.to_string()
            },
            field(&r.our_first),
            r.json.name(),
            r.json_bytes,
            r.gcc_flags.join(" "),
            r.clang_flags.join(" ")
        )?;
    }
    sink.flush()?;

    let path = out.join("disagreements.tsv");
    let mut sink =
        BufWriter::new(fs::File::create(&path).map_err(|e| format!("cannot create {}: {e}", path.display()))?);
    writeln!(
        sink,
        "file\tbegin\tend\tline\tverdict\tclang_kind\tclang_category\tour_kind\tour_category\tcover\tsilent\texcerpt"
    )?;
    for r in results {
        for row in &r.rows {
            writeln!(
                sink,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                field(&r.label),
                row.begin,
                row.end,
                row.line,
                row.verdict.name(),
                row.clang_kind,
                row.clang,
                row.our_kind,
                row.ours.map_or_else(|| "none".to_owned(), |l| l.to_string()),
                row.cover,
                u8::from(!r.our_error()),
                field(&row.excerpt)
            )?;
        }
    }
    sink.flush()?;
    Ok(())
}

/// A class of disagreements with its counts and its first example.
struct Class {
    files: HashSet<usize>,
    count: u64,
    example: (usize, usize),
}

/// Group rows by a key, and give the classes with the most files first.
fn classes(
    results: &[FileResult],
    accept: impl Fn(&FileResult, &super::Row) -> bool,
    key: impl Fn(&super::Row) -> String,
) -> Vec<(String, Class)> {
    let mut map: HashMap<String, Class> = HashMap::new();
    for (file, result) in results.iter().enumerate() {
        for (index, row) in result.rows.iter().enumerate() {
            if !accept(result, row) {
                continue;
            }
            let class = map.entry(key(row)).or_insert_with(|| Class {
                files: HashSet::new(),
                count: 0,
                example: (file, index),
            });
            class.files.insert(file);
            class.count += 1;
        }
    }
    let mut list: Vec<(String, Class)> = map.into_iter().collect();
    list.sort_by(|a, b| {
        (Reverse(a.1.files.len()), Reverse(a.1.count), &a.0).cmp(&(Reverse(b.1.files.len()), Reverse(b.1.count), &b.0))
    });
    list
}

/// Write the lines of a list of classes with one example each.
fn write_classes(text: &mut String, results: &[FileResult], list: &[(String, Class)], top: usize) {
    writeln!(text, "  {:>6} {:>7}  class", "files", "count").expect("a write to a String does not fail");
    for (key, class) in list.iter().take(top) {
        let (file, index) = class.example;
        let result = &results[file];
        let row = &result.rows[index];
        writeln!(
            text,
            "  {:>6} {:>7}  {key}\n                  {}:{}  {} / {}  `{}`",
            class.files.len(),
            class.count,
            result.label,
            row.line,
            row.clang_kind,
            row.our_kind,
            row.excerpt
        )
        .expect("a write to a String does not fail");
    }
}

/// Write the TSV files and `summary.txt` to `out`, and give the summary.
pub fn write(
    out: &Path,
    oracle: &Oracle,
    results: &[FileResult],
    top: usize,
    elapsed: Duration,
) -> Result<String, Box<dyn Error>> {
    fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    write_tables(out, results)?;
    let mut text = String::new();
    let w = &mut text;
    let readable: Vec<&FileResult> = results.iter().filter(|r| r.readable).collect();
    let mut languages: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &readable {
        *languages.entry(r.language.name()).or_default() += 1;
    }
    writeln!(w, "gcc:   {}\nclang: {}", oracle.gcc.version, oracle.clang.version)?;
    writeln!(
        w,
        "files {}  unreadable {}  languages {:?}  wall {:.0} s\n",
        results.len(),
        results.len() - readable.len(),
        languages,
        elapsed.as_secs_f64()
    )?;

    let mut matrix = [[[0u64; 2]; 3]; 3];
    for r in &readable {
        matrix[column(r.gcc.status)][column(r.clang.status)][usize::from(r.our_error())] += 1;
    }
    writeln!(
        w,
        "validity: files with no error in our tree / files with an error in our tree"
    )?;
    writeln!(
        w,
        "  {:<12} {:>16} {:>16} {:>16}",
        "", "clang accept", "clang reject", "clang other"
    )?;
    for (index, name) in ["gcc accept", "gcc reject", "gcc other"].iter().enumerate() {
        let cells: Vec<String> = (0..3)
            .map(|c| format!("{} / {}", matrix[index][c][0], matrix[index][c][1]))
            .collect();
        writeln!(w, "  {name:<12} {:>16} {:>16} {:>16}", cells[0], cells[1], cells[2])?;
    }
    let both_accept = matrix[0][0][0] + matrix[0][0][1];
    writeln!(
        w,
        "  the compilers accept, and our tree has an error: {} of {} ({}%)",
        matrix[0][0][1],
        both_accept,
        percent(matrix[0][0][1], both_accept)
    )?;
    writeln!(
        w,
        "  the compilers reject, and our tree has no error: {} of {}",
        matrix[1][1][0],
        matrix[1][1][0] + matrix[1][1][1]
    )?;
    writeln!(
        w,
        "  the compilers disagree: gcc accepts and clang rejects {}, gcc rejects and clang accepts {}\n",
        matrix[0][1][0] + matrix[0][1][1],
        matrix[1][0][0] + matrix[1][0][1]
    )?;

    let compared = readable.iter().filter(|r| r.compared).count();
    let too_large = readable.iter().filter(|r| r.json == JsonStatus::TooLarge).count();
    let invalid = readable.iter().filter(|r| r.json == JsonStatus::Invalid).count();
    let rejected = readable.iter().filter(|r| r.clang.status != Status::Accept).count();
    writeln!(
        w,
        "structure: {compared} files compared, clang does not accept {rejected}, JSON larger than the limit {too_large}, JSON not valid {invalid}"
    )?;
    let mut totals: BTreeMap<Category, [u64; 5]> = BTreeMap::new();
    for r in &readable {
        for (category, counts) in &r.counts {
            let total = totals.entry(*category).or_default();
            for (sum, count) in total.iter_mut().zip(counts) {
                *sum += u64::from(*count);
            }
        }
    }
    writeln!(
        w,
        "  {:<24} {:<12} {:>8} {:>8} {:>9} {:>7} {:>8} {:>8} {:>7} {:>9}",
        "category", "group", "facts", "agree", "ambiguous", "nested", "mismatch", "missing", "agree%", "explained%"
    )?;
    let mut all = [0u64; 5];
    for category in Category::ALL {
        let Some(counts) = totals.get(category) else { continue };
        let total: u64 = counts.iter().sum();
        for (sum, count) in all.iter_mut().zip(counts) {
            *sum += count;
        }
        let [agree, ambiguous, nested, mismatch, missing] = *counts;
        writeln!(
            w,
            "  {:<24} {:<12} {total:>8} {agree:>8} {ambiguous:>9} {nested:>7} {mismatch:>8} {missing:>8} {:>7} {:>9}",
            category.name(),
            category.group().name(),
            percent(agree, total),
            percent(agree + ambiguous + nested, total)
        )?;
    }
    let total: u64 = all.iter().sum();
    writeln!(
        w,
        "  {:<24} {:<12} {total:>8} {:>8} {:>9} {:>7} {:>8} {:>8} {:>7} {:>9}\n",
        "all",
        "",
        all[0],
        all[1],
        all[2],
        all[3],
        all[4],
        percent(all[0], total),
        percent(all[0] + all[1] + all[2], total)
    )?;

    let silent = classes(
        results,
        |r, row| !r.our_error() && matches!(row.verdict, Verdict::Mismatch | Verdict::Missing),
        |row| match row.verdict {
            Verdict::Missing => format!("missing: clang {} in our {}", row.clang, row.cover),
            _ => format!(
                "mismatch: clang {} / ours {}",
                row.clang,
                row.ours.map_or_else(String::new, |l| l.to_string())
            ),
        },
    );
    writeln!(
        w,
        "silent disagreements in files with no error in our tree: {} classes, by files",
        silent.len()
    )?;
    write_classes(w, results, &silent, top);
    let ambiguous = classes(
        results,
        |_, row| row.verdict == Verdict::Ambiguous,
        |row| {
            format!(
                "clang {} / ours {}",
                row.clang,
                row.ours.map_or_else(String::new, |l| l.to_string())
            )
        },
    );
    let nested: usize = results
        .iter()
        .map(|r| r.rows.iter().filter(|row| row.verdict == Verdict::Nested).count())
        .sum();
    writeln!(
        w,
        "\nambiguous by design: {} classes, and {nested} disagreements nested in their ranges",
        ambiguous.len()
    )?;
    write_classes(w, results, &ambiguous, top);
    let path = out.join("summary.txt");
    fs::write(&path, &text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(text)
}

/// The facts of one byte range in the side-by-side view.
#[derive(Default)]
struct Line {
    clang: Vec<String>,
    ours: Vec<String>,
    verdicts: Vec<Verdict>,
    /// True when one of our facts at the range is not a name, a type, a literal, or an attribute.
    structural: bool,
}

/// Print the side-by-side view of a snippet, and give the number of mismatches and missing nodes.
pub fn show(analysis: &Analysis, tree: Option<&str>) -> usize {
    println!(
        "gcc    {:<8} {}  {}",
        analysis.gcc.status.name(),
        analysis.gcc_flags.join(" "),
        analysis.gcc.error
    );
    println!(
        "clang  {:<8} {}  {}  json {} {} bytes",
        analysis.clang.check.status.name(),
        analysis.clang_flags.join(" "),
        analysis.clang.check.error,
        analysis.clang.json.name(),
        analysis.clang.bytes
    );
    let starts = line_starts(&analysis.source);
    match &analysis.tree {
        None => println!("ours   the time limit stopped the parse"),
        Some(tree) if tree.errors > 0 => {
            let node = tree.node(tree.first_error);
            let (line, column) = position(&starts, node.start);
            println!(
                "ours   {} errors, the first at {line}:{column} {}",
                tree.errors, node.kind
            );
        }
        Some(_) => println!("ours   no error"),
    }
    if !analysis.compared {
        println!("The structure is not compared, because Clang does not accept the snippet.");
    }
    let mut lines: BTreeMap<(u32, Reverse<u32>), Line> = BTreeMap::new();
    for outcome in &analysis.outcomes {
        let fact = outcome.clang;
        let line = lines.entry((fact.begin, Reverse(fact.end))).or_default();
        line.clang.push(format!(
            "{} ({})",
            analysis.clang.nodes[fact.node as usize].kind, fact.label
        ));
        line.verdicts.push(outcome.verdict);
    }
    if let Some(tree) = &analysis.tree {
        for fact in &analysis.our_facts {
            let line = lines.entry((fact.begin, Reverse(fact.end))).or_default();
            line.ours.push(format!("{} ({})", tree.kind(fact.node), fact.label));
            line.structural |= !matches!(
                fact.label.category,
                Category::Name | Category::Type | Category::Literal | Category::Attribute | Category::TypeOperand
            );
        }
    }
    let mut open: Vec<(u32, u32)> = Vec::new();
    let mut disagreements = 0;
    println!("\n{:<44} {:<42} {:<10} text", "range  clang", "ours", "verdict");
    for (&(begin, Reverse(end)), line) in &lines {
        // A range with no Clang fact and only names, types, or literals of ours does not help the reader.
        if line.clang.is_empty() && !line.structural {
            continue;
        }
        while open.last().is_some_and(|&(b, e)| !(b <= begin && end <= e)) {
            open.pop();
        }
        let depth = open.len();
        open.push((begin, end));
        let (row, column) = position(&starts, begin);
        disagreements += line
            .verdicts
            .iter()
            .filter(|v| matches!(v, Verdict::Mismatch | Verdict::Missing))
            .count();
        let verdict = line
            .verdicts
            .iter()
            .max()
            .map_or(String::new(), |v| v.name().to_owned());
        let range = format!("{}{row}:{column} {begin}..{end}", "  ".repeat(depth.min(12)));
        println!(
            "{:<18} {:<25} {:<42} {:<10} {}",
            range,
            line.clang.join(", "),
            line.ours.join(", "),
            verdict,
            excerpt(&analysis.source, begin, end)
        );
    }
    if let Some(tree) = tree {
        println!("\n{tree}");
    }
    disagreements
}
