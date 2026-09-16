//! Write the TSV files and the summary of a run, and print the side-by-side view of one snippet.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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

    // THE TABLE ABOVE IS RECALL. THIS ONE IS THE OTHER HALF, AND THE TWO ANSWER DIFFERENT QUESTIONS.
    // Every row above counts CLANG facts, so a node of ours that Clang states no fact for cannot
    // appear in it. The caution travels with the numbers, in the same output, because a count read
    // without it becomes a defect count in the next message that quotes it.
    let mut ours: BTreeMap<Category, [u64; 3]> = BTreeMap::new();
    for r in &readable {
        for (category, counts) in &r.added {
            let total = ours.entry(*category).or_default();
            for (sum, count) in total.iter_mut().zip(counts) {
                *sum += u64::from(*count);
            }
        }
    }
    let facts: u64 = ours.values().map(|counts| counts[0]).sum();
    let in_macro: u64 = ours.values().map(|counts| counts[1]).sum();
    let edge: u64 = ours.values().map(|counts| counts[2]).sum();
    writeln!(
        w,
        "our facts that no clang fact stands at: {facts}, in a macro definition {in_macro}, at the edge of a clang fact of the same category {edge}\n\
         \x20 THE TABLE ABOVE MEASURES RECALL AND THIS ONE DOES NOT MEASURE PRECISION YET. A row above\n\
         \x20 counts the CLANG facts of a category, so `agree%` says how many of the nodes CLANG states\n\
         \x20 we carry. It says NOTHING about how many of the nodes WE state are correct. This count is\n\
         \x20 the population that answer needs, and it is NOT A COUNT OF DEFECTS. THREE GROUPS IN IT\n\
         \x20 ARE CORRECT BY CONSTRUCTION: the body of a macro definition, which Clang expands before\n\
         \x20 it builds an AST and which the second column counts; a node of ours that Clang's AST\n\
         \x20 states nothing about; and OUR CORRECT NODE AT A DIFFERENT BYTE RANGE, because Clang\n\
         \x20 ends a declaration at a token that is not always the last token of the construct.\n\
         \x20 THE THIRD COLUMN BOUNDS THAT THIRD GROUP AND DOES NOT PROVE IT, IN BOTH DIRECTIONS. It\n\
         \x20 counts a fact of ours where a Clang fact of the SAME category begins or ends at the same\n\
         \x20 byte. Two facts of one category that share a byte by chance make a false yes. A form\n\
         \x20 whose two ranges share NEITHER end makes a false no, and `__extension__` before a\n\
         \x20 declaration is that form. The rule is the one a reader can check by hand, over a tighter\n\
         \x20 one that hides a heuristic.\n\
         \x20 READ IT AS A CLASSIFICATION. A rate over it has no meaning until each group has a name."
    )?;
    writeln!(
        w,
        "  {:<24} {:<12} {:>8} {:>10} {:>8}",
        "category", "group", "ours", "in a macro", "an edge"
    )?;
    for category in Category::ALL {
        let Some(counts) = ours.get(category) else { continue };
        writeln!(
            w,
            "  {:<24} {:<12} {:>8} {:>10} {:>8}",
            category.name(),
            category.group().name(),
            counts[0],
            counts[1],
            counts[2]
        )?;
    }
    writeln!(w)?;

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

/// One row of the category table of a summary.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CategoryRow {
    facts: u64,
    agree: u64,
    percent: f64,
}

/// The number of counts that follow the name in a row of the category table: the facts, the five
/// verdicts, the agree percent, and the explained percent.
const CATEGORY_COUNTS: usize = 8;

/// Read the category table of a `summary.txt`. The key of a row is its name and its group together.
///
/// The name of a category holds a blank, as in `functional cast`, and the `all` row has no group. The
/// reader takes the fields from the right for this reason: the last eight fields are the counts, and
/// each field before them belongs to the name. A line with fewer fields, or with a last field that is
/// not a number, is not a row of the table. O(n) in the length of the file.
fn read_categories(text: &str) -> BTreeMap<String, CategoryRow> {
    let mut rows = BTreeMap::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() <= CATEGORY_COUNTS {
            continue;
        }
        let counts = &fields[fields.len() - CATEGORY_COUNTS..];
        let (Ok(facts), Ok(agree), Ok(percent)) = (
            counts[0].parse::<u64>(),
            counts[1].parse::<u64>(),
            counts[CATEGORY_COUNTS - 2].parse::<f64>(),
        ) else {
            continue;
        };
        let name = fields[..fields.len() - CATEGORY_COUNTS].join(" ");
        rows.insert(name, CategoryRow { facts, agree, percent });
    }
    rows
}

/// Compare the category tables of two runs of the differ, and print each category that moved.
///
/// The gate asks whether a commit makes a category fall, and only this comparison answers it. A
/// category falls when its agree percent decreases. O(n log n) in the number of categories.
pub fn compare(a: &Path, b: &Path) -> Result<(), Box<dyn Error>> {
    let read = |directory: &Path| -> Result<BTreeMap<String, CategoryRow>, Box<dyn Error>> {
        let path = directory.join("summary.txt");
        let text = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let rows = read_categories(&text);
        if rows.is_empty() {
            return Err(format!("{} holds no category table", path.display()).into());
        }
        Ok(rows)
    };
    let (before, after) = (read(a)?, read(b)?);
    println!("{:<34} {:>16} {:>18} {:>16}", "category", "facts", "agree", "agree%");
    let mut fell = Vec::new();
    let mut moved = 0;
    for name in before.keys().chain(after.keys()).collect::<BTreeSet<_>>() {
        let (x, y) = (before.get(name).copied(), after.get(name).copied());
        let (x, y) = (x.unwrap_or_default(), y.unwrap_or_default());
        if x == y {
            continue;
        }
        moved += 1;
        // A category that falls loses agreements for the same facts, or it agrees on a smaller part.
        let fall = y.percent < x.percent;
        if fall && name != "all" {
            fell.push(name.clone());
        }
        println!(
            "{:<34} {:>7}->{:<7} {:>8}->{:<8} {:>7.1}->{:<7.1}{}",
            name,
            x.facts,
            y.facts,
            x.agree,
            y.agree,
            x.percent,
            y.percent,
            if fall { "  <== FALL" } else { "" }
        );
    }
    if moved == 0 {
        println!("no category moved");
    }
    println!("\ncategories that fell: {}", fell.len());
    for name in &fell {
        println!("  {name}");
    }
    if let (Some(x), Some(y)) = (before.get("all"), after.get("all")) {
        let percent = |r: &CategoryRow| 100.0 * r.agree as f64 / r.facts.max(1) as f64;
        println!(
            "all agreement {:.3}% -> {:.3}%, agreements {} -> {} of {} facts",
            percent(x),
            percent(y),
            x.agree,
            y.agree,
            y.facts
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::read_categories;

    /// The reader takes the name of a category from the fields before the counts, so a name with a
    /// blank and a row with no group both give the correct key.
    #[test]
    fn the_reader_of_a_category_table_keeps_a_name_that_holds_a_blank() {
        let text = "\
structure: 5558 files compared, clang does not accept 2061, JSON larger than the limit 84
  category                 group           facts    agree ambiguous  nested mismatch  missing  agree% explained%
  functional cast          expression       1863     1126       730       2        1        4    60.4      99.7
  call                     expression      10148    10101         9       0       37        1    99.5      99.6
  all                                     206072   203951      1584      46      212     279    99.0      99.8
";
        let rows = read_categories(text);
        assert_eq!(rows.len(), 3, "the header and the structure line are not rows");
        let cast = rows.get("functional cast expression").expect("the name holds its group");
        assert_eq!(cast.facts, 1863);
        assert_eq!(cast.agree, 1126);
        assert!((cast.percent - 60.4).abs() < 1e-9);
        let all = rows.get("all").expect("the row with no group");
        assert_eq!(all.facts, 206072);
        assert_eq!(all.agree, 203951);
    }
}
