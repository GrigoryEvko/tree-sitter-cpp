//! Read the language options of a test file, and make the flags of the two compilers.
//!
//! A GCC test gives its options in `dg-options` and `dg-additional-options` lines, and its standards in the
//! target selectors of `dg-do` and `dg-require-effective-target`. A Clang test gives its options in
//! `// RUN:` lines. The flags keep only `-std=`, `-x`, `-f`, `-D`, `-U`, `-I`, and the Clang target. A probe
//! removes each flag that a compiler does not know.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};

use regex::Regex;

/// The language of a test file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    Cxx,
    C,
    Cuda,
}

impl Language {
    /// The name of the language in the report.
    pub fn name(self) -> &'static str {
        match self {
            Language::Cxx => "c++",
            Language::C => "c",
            Language::Cuda => "cuda",
        }
    }
}

/// The flags of a test file for the two compilers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub language: Language,
    pub gcc: Vec<String>,
    pub clang: Vec<String>,
}

/// The options that a test file gives, before they become the flags of a compiler.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Given {
    standard: Option<String>,
    language: Option<Language>,
    target: Option<String>,
    /// The `-f`, `-D`, `-U`, and `-I` flags.
    flags: Vec<String>,
}

/// The GCC options lines: the kind, the options, and the optional target selector.
static DG_OPTIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"dg-(?:additional-)?options\s+"([^"]*)"(?:\s*\{([^}]*)\})?"#).expect("the expression is valid")
});
/// The target selectors of a GCC test.
static DG_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"dg-do\s+\w+\s*\{\s*target\s+([^}]*)\}|dg-require-effective-target\s+([^\s}]+)")
        .expect("the expression is valid")
});
/// A standard with a limit in a target selector.
static SELECTOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(!\s*)?c\+\+(\d\d)(_only|_down)?").expect("the expression is valid"));

/// The C++ standards in order, with the name of each one for GCC and for Clang.
const STANDARDS: &[(&str, &str, &str)] = &[
    ("98", "c++98", "c++98"),
    ("11", "c++11", "c++11"),
    ("14", "c++14", "c++14"),
    ("17", "c++17", "c++17"),
    ("20", "c++20", "c++20"),
    ("23", "c++23", "c++23"),
    ("26", "c++26", "c++2c"),
];

/// The default C++ standard for GCC.
pub const GCC_DEFAULT: &str = "c++26";
/// The default C++ standard for Clang.
pub const CLANG_DEFAULT: &str = "c++2c";

/// The language of a `-x` value.
fn language_of(value: &str) -> Option<Language> {
    match value {
        "c++" | "c++-header" | "c++-module" | "c++-system-header" | "c++-user-header" => Some(Language::Cxx),
        "c" | "c-header" => Some(Language::C),
        "cuda" => Some(Language::Cuda),
        _ => None,
    }
}

/// The name of a standard for GCC.
fn gcc_standard(value: &str) -> String {
    match value {
        "c++2c" => "c++26".to_owned(),
        "gnu++2c" => "gnu++26".to_owned(),
        other => other.to_owned(),
    }
}

/// The name of a standard for Clang.
fn clang_standard(value: &str) -> String {
    match value {
        "c++26" | "c++2d" | "c++29" => "c++2c".to_owned(),
        "gnu++26" | "gnu++2d" | "gnu++29" => "gnu++2c".to_owned(),
        other => other.to_owned(),
    }
}

/// The newest standard that the target selectors of a GCC test permit, if they give a limit.
fn selector_standard(source: &str) -> Option<&'static str> {
    let mut limit: Option<usize> = None;
    for captures in DG_TARGET.captures_iter(source) {
        let selectors = captures.get(1).or_else(|| captures.get(2)).map_or("", |m| m.as_str());
        for selector in SELECTOR.captures_iter(selectors) {
            let Some(position) = STANDARDS.iter().position(|(year, _, _)| *year == &selector[2]) else {
                continue;
            };
            let newest = if selector.get(1).is_some() {
                position.checked_sub(1)
            } else if selector.get(3).is_some() {
                Some(position)
            } else {
                None
            };
            if let Some(newest) = newest {
                limit = Some(limit.map_or(newest, |l| l.min(newest)));
            }
        }
    }
    limit.map(|l| STANDARDS[l].1)
}

/// True when a GCC target selector applies to an x86_64 Linux host or is a language selector.
fn selector_applies(selector: &str) -> bool {
    ["x86_64", "i?86", "*-*-*", "linux", "c++", "native"]
        .iter()
        .any(|word| selector.contains(word))
}

/// Add a flag of a test to `given`. `next` gives the value of a flag with a separate value.
fn add_flag<'a>(given: &mut Given, flag: &'a str, next: &mut impl Iterator<Item = &'a str>, directory: &str) {
    let path = |value: &str| value.replace("%S", directory);
    match flag {
        "-x" => given.language = next.next().and_then(language_of).or(given.language),
        "-triple" | "-target" => given.target = next.next().map(str::to_owned),
        "-I" | "-D" | "-U" => {
            if let Some(value) = next.next()
                && !path(value).contains('%')
            {
                given.flags.push(format!("{flag}{}", path(value)));
            }
        }
        _ if flag.starts_with("-std=") => given.standard = Some(flag["-std=".len()..].to_owned()),
        _ if flag.starts_with("-x") => given.language = language_of(&flag[2..]).or(given.language),
        _ if flag.starts_with("-triple=") || flag.starts_with("--target=") || flag.starts_with("-target=") => {
            given.target = flag.split_once('=').map(|(_, value)| value.to_owned());
        }
        _ if flag.starts_with("-I") || flag.starts_with("-D") || flag.starts_with("-U") => {
            if !path(flag).contains('%') {
                given.flags.push(path(flag));
            }
        }
        // Delayed template parsing removes the bodies of templates from the AST. A module flag makes a module cache necessary.
        // The oracle gives its own flags for the output and for the diagnostics.
        _ if flag.starts_with("-fdelayed-template-parsing")
            || (flag.starts_with("-fmodule") && flag != "-fmodules-ts")
            || flag == "-fsyntax-only"
            || flag.contains("color-diagnostics")
            || flag.starts_with("-fdiagnostics-")
            || flag.starts_with("-fcaret-diagnostics") => {}
        _ if flag.starts_with("-f") => given.flags.push(flag.to_owned()),
        _ => {}
    }
}

/// The options of the `// RUN:` lines of a Clang test, or `None` when the file has no Clang RUN line.
///
/// A RUN line that ends with `\` continues on the next RUN line. The first RUN line with a standard wins,
/// and the first RUN line wins when no line has a standard.
fn run_options(source: &str, directory: &str) -> Option<Given> {
    let mut logical: Vec<String> = Vec::new();
    let mut open = false;
    for line in source.lines() {
        let Some((_, rest)) = line.split_once("RUN:") else {
            open = false;
            continue;
        };
        let rest = rest.trim();
        let (text, continues) = match rest.strip_suffix('\\') {
            Some(text) => (text, true),
            None => (rest, false),
        };
        match logical.last_mut() {
            Some(last) if open => {
                last.push(' ');
                last.push_str(text);
            }
            _ => logical.push(text.to_owned()),
        }
        open = continues;
    }
    let mut best: Option<Given> = None;
    for line in &logical {
        let words: Vec<&str> = line
            .split_whitespace()
            .map(|w| w.trim_matches(|c| c == '\'' || c == '"'))
            .collect();
        let Some(start) = words
            .iter()
            .position(|w| matches!(*w, "%clang_cc1" | "%clang" | "%clangxx" | "%clang++"))
        else {
            continue;
        };
        let mut given = Given::default();
        let mut rest = words[start + 1..]
            .iter()
            .copied()
            .take_while(|w| !matches!(*w, "|" | "||" | "&&" | ";" | ">" | "2>&1" | "<" | "2>"));
        while let Some(word) = rest.next() {
            add_flag(&mut given, word, &mut rest, directory);
        }
        let better = match &best {
            None => true,
            Some(current) => current.standard.is_none() && given.standard.is_some(),
        };
        if better {
            best = Some(given);
        }
    }
    best
}

/// The options of the `dg-options` and `dg-additional-options` lines of a GCC test.
fn dg_options(source: &str, directory: &str) -> Given {
    let mut given = Given::default();
    for captures in DG_OPTIONS.captures_iter(source) {
        if captures
            .get(2)
            .is_some_and(|selector| !selector_applies(selector.as_str()))
        {
            continue;
        }
        let mut words = captures[1].split_whitespace();
        while let Some(word) = words.next() {
            add_flag(&mut given, word, &mut words, directory);
        }
    }
    if given.standard.is_none() {
        given.standard = selector_standard(source).map(str::to_owned);
    }
    given
}

/// Read the options of a test file, and make the flags of the two compilers.
pub fn read(path: &Path, source: &str) -> Options {
    let directory = path
        .parent()
        .map_or(String::new(), |p| p.to_string_lossy().into_owned());
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let shared = path.to_string_lossy().contains("c-c++-common");
    let by_extension = match extension {
        "c" | "h" if !shared => Language::C,
        "cu" => Language::Cuda,
        _ => Language::Cxx,
    };
    let given = run_options(source, &directory).unwrap_or_else(|| dg_options(source, &directory));
    let standard = given.standard.clone();
    let language = match (&given.language, &standard) {
        (Some(language), _) => *language,
        (None, Some(standard)) if standard.contains("++") => {
            if by_extension == Language::Cuda {
                Language::Cuda
            } else {
                Language::Cxx
            }
        }
        (None, Some(_)) => Language::C,
        (None, None) => by_extension,
    };
    let mut gcc = Vec::new();
    let mut clang = Vec::new();
    match (language, &standard) {
        (_, Some(standard)) => {
            gcc.push(format!("-std={}", gcc_standard(standard)));
            clang.push(format!("-std={}", clang_standard(standard)));
        }
        (Language::Cxx | Language::Cuda, None) => {
            gcc.push(format!("-std={GCC_DEFAULT}"));
            clang.push(format!("-std={CLANG_DEFAULT}"));
        }
        (Language::C, None) => {}
    }
    match language {
        Language::Cxx if by_extension != Language::Cxx || shared => {
            gcc.push("-xc++".to_owned());
            clang.push("-xc++".to_owned());
        }
        Language::C if by_extension != Language::C => {
            gcc.push("-xc".to_owned());
            clang.push("-xc".to_owned());
        }
        Language::Cuda => {
            gcc.push("-xc++".to_owned());
            clang.extend(["-xcuda".to_owned(), "-nocudainc".to_owned(), "-nocudalib".to_owned()]);
        }
        _ => {}
    }
    if let Some(target) = &given.target {
        clang.push(format!("--target={target}"));
    }
    gcc.extend(given.flags.iter().cloned());
    clang.extend(given.flags.iter().cloned());
    Options { language, gcc, clang }
}

/// The key of a probe: the compiler, the flag, and the language.
type ProbeKey = (String, String, Language);

/// The results of the probes of flags.
static PROBES: LazyLock<Mutex<HashMap<ProbeKey, bool>>> = LazyLock::new(Default::default);

/// True when a compiler accepts a flag for a language. The first probe of each flag runs the compiler.
///
/// The probe runs in `cwd`. A dump flag such as `-fstack-usage` writes a file in the directory where the
/// compiler runs, and that directory must not be the directory of the user.
fn accepts(binary: &str, flag: &str, language: Language, cwd: &Path) -> bool {
    let key = (binary.to_owned(), flag.to_owned(), language);
    if let Some(&known) = PROBES.lock().expect("no probe panics").get(&key) {
        return known;
    }
    let x = if language == Language::C { "c" } else { "c++" };
    let accepted = Command::new(binary)
        .args(["-fsyntax-only", "-x", x, flag, "/dev/null"])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    PROBES.lock().expect("no probe panics").insert(key, accepted);
    accepted
}

/// The flags that a compiler accepts. A rejected standard becomes the default standard of the language.
///
/// The probes run in `cwd`. O(n) in the number of flags, plus one compiler start for each flag that no probe knows.
pub fn filter(binary: &str, flags: &[String], language: Language, default_standard: &str, cwd: &Path) -> Vec<String> {
    let mut out = Vec::with_capacity(flags.len());
    for flag in flags {
        let unprobed = ["-x", "-D", "-U", "-I", "-nocuda"].iter().any(|p| flag.starts_with(p));
        if unprobed || accepts(binary, flag, language, cwd) {
            out.push(flag.clone());
        } else if flag.starts_with("-std=") && language != Language::C {
            out.push(format!("-std={default_standard}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flag_probe_runs_the_compiler_in_the_given_directory() {
        if Command::new("g++").arg("--version").output().is_err() {
            return;
        }
        let directory = std::env::temp_dir().join(format!("differ-probe-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("the temporary directory is writable");
        let accepted = accepts("g++", "-fstack-usage", Language::Cxx, &directory);
        let written = directory.join("a-null.su").exists();
        let _ = std::fs::remove_dir_all(&directory);
        assert!(accepted, "g++ does not accept `-fstack-usage`");
        assert!(written, "the probe did not write `a-null.su` in its directory");
    }

    #[test]
    fn gcc_options_and_selectors_give_the_flags_and_the_standard() {
        let source = "// { dg-do compile { target c++17_down } }\n\
                      // { dg-options \"-fconcepts -DX=1 -Wall\" }\n\
                      // { dg-additional-options \"-mavx\" { target arm*-*-* } }\n\
                      // { dg-additional-options \"-fno-rtti\" }\n";
        let options = read(Path::new("/g/g++.dg/a.C"), source);
        assert_eq!(options.language, Language::Cxx);
        assert_eq!(options.gcc, ["-std=c++17", "-fconcepts", "-DX=1", "-fno-rtti"]);
        assert_eq!(options.clang, ["-std=c++17", "-fconcepts", "-DX=1", "-fno-rtti"]);
    }

    #[test]
    fn a_negative_selector_gives_the_standard_before_it_and_a_shared_c_file_is_cxx() {
        let source = "/* { dg-do compile { target { ! c++11 } } } */\n";
        let options = read(Path::new("/g/c-c++-common/b.c"), source);
        assert_eq!(options.language, Language::Cxx);
        assert_eq!(options.gcc, ["-std=c++98", "-xc++"]);
    }

    #[test]
    fn a_file_with_no_options_gets_the_default_standards() {
        let options = read(Path::new("/g/c.C"), "int x;\n");
        assert_eq!(options.gcc, ["-std=c++26"]);
        assert_eq!(options.clang, ["-std=c++2c"]);
    }

    #[test]
    fn run_lines_continue_and_give_the_standard_the_target_and_the_paths() {
        let source = "// RUN: %clang_cc1 -fsyntax-only -verify %s\n\
                      // RUN: %clang_cc1 -triple x86_64-pc-win32 -fms-extensions \\\n\
                      // RUN:   -std=c++20 -I %S/Inputs -fdelayed-template-parsing %s | FileCheck %s\n";
        let options = read(Path::new("/l/test/SemaCXX/d.cpp"), source);
        assert_eq!(options.language, Language::Cxx);
        assert_eq!(
            options.gcc,
            ["-std=c++20", "-fms-extensions", "-I/l/test/SemaCXX/Inputs"]
        );
        assert_eq!(
            options.clang,
            [
                "-std=c++20",
                "--target=x86_64-pc-win32",
                "-fms-extensions",
                "-I/l/test/SemaCXX/Inputs"
            ]
        );
    }

    #[test]
    fn a_run_line_with_x_cxx_makes_a_c_file_cxx_and_the_standard_names_differ() {
        let source = "// RUN: %clang_cc1 -x c++ -std=c++2c -fsyntax-only %s\n";
        let options = read(Path::new("/l/test/e.c"), source);
        assert_eq!(options.language, Language::Cxx);
        assert_eq!(options.gcc, ["-std=c++26", "-xc++"]);
        assert_eq!(options.clang, ["-std=c++2c", "-xc++"]);
    }
}
