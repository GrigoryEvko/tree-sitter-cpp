//! Run the compilers on a file with a time limit, record the first error, and cache the results.
//!
//! The cache holds the result of each compiler run and the Clang nodes of the main file, not the raw JSON.
//! A dump with standard headers can have hundreds of megabytes, and the work directory is in memory.
//!
//! Each compiler runs in its own process group. The GCC driver runs `cc1plus` as a child process, so a
//! time limit stops the full group. Otherwise `cc1plus` keeps the standard error pipe open and the run waits.

use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::clang::{self, ClangNode};
use super::json::JsonError;

/// The version of the cache format. A change of the reader in `clang.rs` or of the node format must change it.
const CACHE_FORMAT: &str = "differ-cache 4";
/// The maximum number of bytes of standard error that a run keeps.
const STDERR_BYTES: usize = 64 * 1024;
/// The maximum length of a first error.
const ERROR_BYTES: usize = 200;

/// A compiler binary and the first line of its version.
pub struct Compiler {
    pub binary: String,
    pub version: String,
}

impl Compiler {
    /// Find the version of a compiler binary.
    pub fn new(binary: &str) -> Result<Compiler, String> {
        let output = Command::new(binary)
            .arg("--version")
            .output()
            .map_err(|e| format!("cannot run {binary} --version: {e}. Install the compiler or change its path."))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let version = text.lines().next().unwrap_or_default().trim().to_owned();
        Ok(Compiler {
            binary: binary.to_owned(),
            version,
        })
    }
}

/// The result of a compiler run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Status {
    Accept,
    Reject,
    Timeout,
    Crash,
    /// The compiler did not start.
    Unavailable,
}

impl Status {
    /// The name of the status in the report.
    pub fn name(self) -> &'static str {
        match self {
            Status::Accept => "accept",
            Status::Reject => "reject",
            Status::Timeout => "timeout",
            Status::Crash => "crash",
            Status::Unavailable => "unavailable",
        }
    }

    /// The status with a name.
    fn parse(name: &str) -> Option<Status> {
        [
            Status::Accept,
            Status::Reject,
            Status::Timeout,
            Status::Crash,
            Status::Unavailable,
        ]
        .into_iter()
        .find(|s| s.name() == name)
    }
}

/// The status of the JSON AST of a Clang run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum JsonStatus {
    Read,
    TooLarge,
    Invalid,
    Absent,
}

impl JsonStatus {
    /// The name of the status in the report.
    pub fn name(self) -> &'static str {
        match self {
            JsonStatus::Read => "read",
            JsonStatus::TooLarge => "too-large",
            JsonStatus::Invalid => "invalid",
            JsonStatus::Absent => "absent",
        }
    }

    /// The status with a name.
    fn parse(name: &str) -> Option<JsonStatus> {
        [
            JsonStatus::Read,
            JsonStatus::TooLarge,
            JsonStatus::Invalid,
            JsonStatus::Absent,
        ]
        .into_iter()
        .find(|s| s.name() == name)
    }
}

/// The validity result of one compiler run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub status: Status,
    /// The first line of standard error with `error:`, without the path of the file.
    pub error: String,
}

/// The result of a Clang run with the JSON AST.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClangRun {
    pub check: Check,
    pub json: JsonStatus,
    pub bytes: u64,
    pub nodes: Vec<ClangNode>,
}

/// The compilers and the settings of the runs.
pub struct Oracle {
    pub gcc: Compiler,
    pub clang: Compiler,
    pub work: PathBuf,
    pub timeout: Duration,
    /// The maximum size of a JSON AST in bytes.
    pub limit: u64,
    pub cache: bool,
}

/// A key of 128 bits for a list of parts.
fn cache_key(parts: &[&[u8]]) -> String {
    let mut first = DefaultHasher::new();
    let mut second = DefaultHasher::new();
    0x9e37_79b9u32.hash(&mut first);
    0x85eb_ca6bu32.hash(&mut second);
    for part in parts {
        part.hash(&mut first);
        part.hash(&mut second);
    }
    format!("{:016x}{:016x}", first.finish(), second.finish())
}

/// Stop a child process and all the processes of its process group.
///
/// The child must be the leader of its group: the command of the child calls `process_group(0)`.
fn kill_group(child: &mut Child) {
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

/// Wait for a child process until it stops or the time limit is over. `None` when the limit stops it.
fn wait(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    let mut pause = Duration::from_millis(1);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => return None,
        }
        if Instant::now() >= deadline {
            kill_group(child);
            return None;
        }
        thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_millis(25));
    }
}

/// Read a stream to its end, and keep its first [`STDERR_BYTES`] bytes.
fn read_capped(mut stream: impl Read) -> String {
    let mut kept = Vec::new();
    let mut buffer = [0u8; 8192];
    while let Ok(count) = stream.read(&mut buffer) {
        if count == 0 {
            break;
        }
        if kept.len() < STDERR_BYTES {
            kept.extend_from_slice(&buffer[..count.min(STDERR_BYTES - kept.len())]);
        }
    }
    String::from_utf8_lossy(&kept).into_owned()
}

/// The first line of standard error with `error:`, without the path of the file, cut to [`ERROR_BYTES`].
fn first_error(stderr: &str, path: &str) -> String {
    let Some(line) = stderr.lines().find(|line| line.contains("error:")) else {
        return String::new();
    };
    let mut line = line
        .strip_prefix(path)
        .unwrap_or(line)
        .trim_start_matches(':')
        .replace('\t', " ");
    if line.len() > ERROR_BYTES {
        let mut cut = ERROR_BYTES;
        while !line.is_char_boundary(cut) {
            cut -= 1;
        }
        line.truncate(cut);
    }
    line
}

/// The status of an exit.
fn status_of(exit: Option<ExitStatus>) -> Status {
    match exit {
        None => Status::Timeout,
        Some(exit) if exit.success() => Status::Accept,
        Some(exit) if exit.code().is_some() => Status::Reject,
        Some(_) => Status::Crash,
    }
}

impl Oracle {
    /// The path of a cache entry.
    fn cache_path(&self, key: &str, extension: &str) -> PathBuf {
        self.work
            .join("cache")
            .join(&key[..2])
            .join(format!("{key}.{extension}"))
    }

    /// Write a cache entry through a temporary file, so that a reader never sees a part of it.
    fn store(&self, path: &Path, text: &str) {
        let Some(directory) = path.parent() else { return };
        if fs::create_dir_all(directory).is_err() {
            return;
        }
        let temporary = path.with_extension(format!("tmp{:?}", thread::current().id()).replace(['(', ')'], ""));
        if fs::write(&temporary, text).is_ok() && fs::rename(&temporary, path).is_err() {
            let _ = fs::remove_file(&temporary);
        }
    }

    /// The directory where the compilers and the flag probes run. GCC writes its module files and dump files there.
    pub fn cwd(&self) -> PathBuf {
        let directory = self.work.join("cwd");
        let _ = fs::create_dir_all(&directory);
        directory
    }

    /// A command for a compiler in its own process group, with no standard input.
    fn command(&self, binary: &str, base: &[&str], flags: &[String], path: &Path) -> Command {
        let mut command = Command::new(binary);
        command
            .args(base)
            .args(flags)
            .arg(path)
            .current_dir(self.cwd())
            .stdin(Stdio::null())
            .process_group(0);
        command
    }

    /// Run GCC with `-fsyntax-only` on a file.
    pub fn gcc(&self, path: &Path, flags: &[String], source: &[u8]) -> Check {
        let key = cache_key(&[b"gcc", self.gcc.version.as_bytes(), flags.join("\0").as_bytes(), source]);
        let entry = self.cache_path(&key, "gcc");
        if self.cache
            && let Ok(text) = fs::read_to_string(&entry)
            && let Some((check, _, _, _)) = decode(&text)
        {
            return check;
        }
        let check = self.run_check(
            &self.gcc.binary,
            &["-fsyntax-only", "-w", "-fdiagnostics-color=never"],
            flags,
            path,
        );
        if self.cache && !matches!(check.status, Status::Unavailable | Status::Timeout) {
            self.store(&entry, &encode(&check, JsonStatus::Absent, 0, &[]));
        }
        check
    }

    /// Run a compiler with no output but standard error, and give its status and first error.
    fn run_check(&self, binary: &str, base: &[&str], flags: &[String], path: &Path) -> Check {
        let spawned = self
            .command(binary, base, flags, path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                return Check {
                    status: Status::Unavailable,
                    error: format!("cannot run {binary}: {error}"),
                };
            }
        };
        let stderr = child.stderr.take().expect("standard error is piped");
        let (exit, text) = thread::scope(|scope| {
            let errors = scope.spawn(|| read_capped(stderr));
            let exit = wait(&mut child, self.timeout);
            (exit, errors.join().unwrap_or_default())
        });
        Check {
            status: status_of(exit),
            error: first_error(&text, &path.to_string_lossy()),
        }
    }

    /// Run Clang with `-fsyntax-only` and the JSON AST dump on a file, and read the nodes of the file.
    pub fn clang(&self, path: &Path, flags: &[String], source: &[u8]) -> ClangRun {
        let key = cache_key(&[
            b"clang",
            CACHE_FORMAT.as_bytes(),
            self.clang.version.as_bytes(),
            flags.join("\0").as_bytes(),
            &self.limit.to_le_bytes(),
            source,
        ]);
        let entry = self.cache_path(&key, "clang");
        if self.cache
            && let Ok(text) = fs::read_to_string(&entry)
            && let Some((check, json, bytes, nodes)) = decode(&text)
        {
            return ClangRun {
                check,
                json,
                bytes,
                nodes,
            };
        }
        let base = [
            "-fsyntax-only",
            "-w",
            "-fno-color-diagnostics",
            "-fno-delayed-template-parsing",
            "-Xclang",
            "-ast-dump=json",
        ];
        let main = path.to_string_lossy().into_owned();
        let spawned = self
            .command(&self.clang.binary, &base, flags, path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                return ClangRun {
                    check: Check {
                        status: Status::Unavailable,
                        error: format!("cannot run {}: {error}", self.clang.binary),
                    },
                    json: JsonStatus::Absent,
                    bytes: 0,
                    nodes: Vec::new(),
                };
            }
        };
        let stdout = child.stdout.take().expect("standard output is piped");
        let stderr = child.stderr.take().expect("standard error is piped");
        let (exit, text, parsed) = thread::scope(|scope| {
            let reader =
                scope.spawn(|| clang::read(BufReader::with_capacity(1 << 20, stdout), &main, source, self.limit));
            let errors = scope.spawn(|| read_capped(stderr));
            let exit = wait(&mut child, self.timeout);
            let parsed = reader.join();
            (exit, errors.join().unwrap_or_default(), parsed)
        });
        let mut run = match parsed {
            Ok(Ok((nodes, bytes))) => ClangRun {
                check: Check {
                    status: status_of(exit),
                    error: first_error(&text, &main),
                },
                json: JsonStatus::Read,
                bytes,
                nodes,
            },
            Ok(Err(JsonError::TooLarge)) => {
                let check = self.run_check(&self.clang.binary, &base[..4], flags, path);
                ClangRun {
                    check,
                    json: JsonStatus::TooLarge,
                    bytes: self.limit,
                    nodes: Vec::new(),
                }
            }
            Ok(Err(_)) | Err(_) => ClangRun {
                check: Check {
                    status: status_of(exit),
                    error: first_error(&text, &main),
                },
                json: JsonStatus::Invalid,
                bytes: 0,
                nodes: Vec::new(),
            },
        };
        if run.check.status != Status::Accept {
            run.nodes.clear();
        }
        if self.cache && !matches!(run.check.status, Status::Unavailable | Status::Timeout) {
            self.store(&entry, &encode(&run.check, run.json, run.bytes, &run.nodes));
        }
        run
    }
}

/// Replace the tabs, the line ends, and the backslashes of a field with escapes.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

/// Replace the escapes of [`escape`] with their characters.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// The text of a cache entry.
fn encode(check: &Check, json: JsonStatus, bytes: u64, nodes: &[ClangNode]) -> String {
    let mut text = format!(
        "{CACHE_FORMAT}\n{}\t{}\t{}\t{}\n",
        check.status.name(),
        json.name(),
        bytes,
        escape(&check.error)
    );
    for node in nodes {
        writeln!(
            text,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            node.kind,
            escape(&node.detail),
            node.begin,
            node.end,
            node.last,
            u8::from(node.plain),
            node.parent,
            u8::from(node.direct),
            node.pos,
            node.siblings,
            node.children,
            node.flags
        )
        .expect("a write to a String does not fail");
    }
    text
}

/// The contents of a cache entry, or `None` when the entry has a different format or is damaged.
fn decode(text: &str) -> Option<(Check, JsonStatus, u64, Vec<ClangNode>)> {
    let mut lines = text.lines();
    if lines.next()? != CACHE_FORMAT {
        return None;
    }
    let mut head = lines.next()?.splitn(4, '\t');
    let status = Status::parse(head.next()?)?;
    let json = JsonStatus::parse(head.next()?)?;
    let bytes = head.next()?.parse().ok()?;
    let error = unescape(head.next().unwrap_or_default());
    let mut nodes = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        let [
            kind,
            detail,
            begin,
            end,
            last,
            plain,
            parent,
            direct,
            pos,
            siblings,
            children,
            flags,
        ] = fields[..]
        else {
            return None;
        };
        nodes.push(ClangNode {
            kind: kind.into(),
            detail: unescape(detail).into(),
            begin: begin.parse().ok()?,
            end: end.parse().ok()?,
            last: last.parse().ok()?,
            plain: plain == "1",
            parent: parent.parse().ok()?,
            direct: direct == "1",
            pos: pos.parse().ok()?,
            siblings: siblings.parse().ok()?,
            children: children.parse().ok()?,
            flags: flags.parse().ok()?,
        });
    }
    Some((Check { status, error }, json, bytes, nodes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cache_entry_reads_back_to_the_same_result() {
        let check = Check {
            status: Status::Reject,
            error: ":3:1: error: a\tb \\ c".to_owned(),
        };
        let nodes = vec![ClangNode {
            kind: "DeclRefExpr".into(),
            detail: "operator\t+".into(),
            begin: 1,
            end: 2,
            last: 1,
            plain: true,
            parent: clang::NONE,
            direct: false,
            pos: 3,
            siblings: 4,
            children: 0,
            flags: 5,
        }];
        let text = encode(&check, JsonStatus::Read, 77, &nodes);
        assert_eq!(decode(&text), Some((check, JsonStatus::Read, 77, nodes)));
    }

    #[test]
    fn the_first_error_has_no_path() {
        let stderr = "/x/a.C:1:2: warning: w\n/x/a.C:3:4: error: expected ';'\n";
        assert_eq!(first_error(stderr, "/x/a.C"), "3:4: error: expected ';'");
    }

    #[test]
    fn a_time_limit_stops_a_grandchild_that_holds_the_pipe() {
        let oracle = Oracle {
            gcc: Compiler {
                binary: "sh".to_owned(),
                version: String::new(),
            },
            clang: Compiler {
                binary: "sh".to_owned(),
                version: String::new(),
            },
            work: std::env::temp_dir().join(format!("differ-test-{}", std::process::id())),
            timeout: Duration::from_millis(300),
            limit: 0,
            cache: false,
        };
        let started = Instant::now();
        // `sh -c SCRIPT NAME`: the script starts `sleep` as a child, and the child keeps the pipe open.
        let check = oracle.run_check("sh", &["-c", "sleep 30 & wait"], &[], Path::new("differ-test"));
        assert_eq!(check.status, Status::Timeout);
        assert!(started.elapsed() < Duration::from_secs(10));
        let _ = fs::remove_dir_all(&oracle.work);
    }
}
