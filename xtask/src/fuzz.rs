//! Parse small changes of the test inputs and of corpus files in child processes, and record each input whose
//! parse does not stop, stops the process, or gives a tree with incorrect ranges.
//!
//! The time limit of `xtask corpus` cannot stop an external scanner that does not return: the runtime calls the
//! progress callback only between the tokens. For this reason, each parse runs in a child process, a copy of
//! xtask with the hidden task `fuzz-worker`. The parent sends one input at a time to the child and waits for the
//! reply with a time limit. When the limit ends, the parent stops the child, and a new child parses the input
//! again with four times the limit. When that limit also ends, the parent records the input as a timeout. When the
//! second parse gives a reply, the parent records the input as slow, and the run does not fail for it. After three
//! timeouts of the changes of one input, a timeout gets no second parse. When the
//! child stops without a reply, for example at a failed assertion of the loop guard in `src/scanner.c` or of the
//! runtime, the parent records the input and the last line of the standard error of the child.
//!
//! The inputs are the snippets of `test/syntax`, the inputs of the examples of `test/corpus`, and the files of an
//! optional list. The changes of an input are:
//! - A cut at the start or at the end of each token
//! - The removal of one token
//! - An insertion of one text of `INSERTIONS` at the start or at the end of each token
//! - A copy of one line before that line
//! - The removal of one line break, which puts two lines together.
//!
//! A token is a word of letters, digits, `_`, `$`, and bytes that are not ASCII, or one other byte that is not
//! white space. When an input has more changes than its limit, the seed selects the changes. The output
//! directory gets `failures.tsv` and one file with the bytes of each failed input.

use std::collections::HashSet;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tree_sitter::{Language, Tree};

use crate::corpus::new_parser;

const USAGE: &str = "\
usage: cargo xtask fuzz [--seconds N] [--seed S] [--jobs J] [--list LIST [--root ROOT]] [--file-changes M] OUT

  --seconds N       The time limit of one parse in a child process. The value is 5. After a timeout, a new
                    child parses the input again with 4 times the limit.
  --seed S          The seed that selects the changes of an input with more changes than its limit. The value
                    is 20260915.
  --jobs J          The number of child processes. The value is the number of processors, with a maximum of 64.
  --list LIST       A file with one path on each line. The changes of these files are also parsed. A relative
                    path is relative to ROOT.
  --file-changes M  The maximum number of changes of each file of LIST. The value is 32.
  OUT               The directory for failures.tsv and for the bytes of each failed input.";

/// The texts that a change inserts at the start or at the end of a token.
const INSERTIONS: [&str; 18] = [
    "{", "}", "(", ")", "[", "]", "<", ">", ";", ",", "#", "\n", "\\", "\"", "'", "/*", "*/", "R\"(",
];
/// The time limit of one parse in a child process.
const DEFAULT_SECONDS: u64 = 5;
/// After a timeout, a new child parses the input again with this number of times the time limit.
const RETRY_FACTOR: u32 = 4;
/// After this number of timeouts of the changes of one input, a timeout of the next change gets no second parse.
const MAX_RETRIED_TIMEOUTS: usize = 3;
/// The seed of the selection of changes.
const DEFAULT_SEED: u64 = 20_260_915;
/// The maximum number of child processes when `--jobs` is not given.
const MAX_DEFAULT_JOBS: usize = 64;
/// The maximum number of changes of a snippet or of the input of an example.
const TEST_INPUT_CHANGES: usize = 4096;
/// The maximum number of changes of a file of the list when `--file-changes` is not given.
const DEFAULT_FILE_CHANGES: usize = 32;
/// The number of bytes of the standard error of a child that the parent keeps.
const STDERR_TAIL: usize = 8192;
/// The limit of the virtual memory of a child in KB: 4 GB. With 32 children, the fuzz task uses a maximum of
/// 128 GB. The parse of the largest corpus file, 3.9 MB, needs much less.
const MEMORY_LIMIT_KB: u64 = 4 * 1024 * 1024;
/// The parent writes the bytes of this number of failed inputs to files. `failures.tsv` has all failures.
const MAX_FAILURE_FILES: usize = 1000;
/// The reply of a child for a tree with correct ranges.
const REPLY_OK: u8 = 0;
/// The reply of a child for a tree with an incorrect range. A message follows.
const REPLY_TREE: u8 = 1;

/// An input and the maximum number of its changes.
struct Input {
    name: String,
    bytes: Vec<u8>,
    limit: usize,
}

/// A small change of an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    /// Remove the bytes from this position to the end.
    Cut(u32),
    /// Remove the bytes of a token.
    Remove { start: u32, end: u32 },
    /// Insert a text of `INSERTIONS` at a position.
    Insert { at: u32, text: u8 },
    /// Insert a copy of the line from `start` to `end`, and a line break, before the line.
    CopyLine { start: u32, end: u32 },
    /// Remove the line break at a position.
    JoinLines(u32),
}

impl Change {
    /// The input with the change. O(n) in the bytes of the input.
    fn apply(self, source: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(source.len() + 8);
        match self {
            Change::Cut(at) => out.extend_from_slice(&source[..at as usize]),
            Change::Remove { start, end } => {
                out.extend_from_slice(&source[..start as usize]);
                out.extend_from_slice(&source[end as usize..]);
            }
            Change::Insert { at, text } => {
                out.extend_from_slice(&source[..at as usize]);
                out.extend_from_slice(INSERTIONS[usize::from(text)].as_bytes());
                out.extend_from_slice(&source[at as usize..]);
            }
            Change::CopyLine { start, end } => {
                out.extend_from_slice(&source[..start as usize]);
                out.extend_from_slice(&source[start as usize..end as usize]);
                out.push(b'\n');
                out.extend_from_slice(&source[start as usize..]);
            }
            Change::JoinLines(at) => {
                out.extend_from_slice(&source[..at as usize]);
                out.extend_from_slice(&source[at as usize + 1..]);
            }
        }
        out
    }

    /// A text that tells the change, for `failures.tsv`.
    fn describe(self) -> String {
        match self {
            Change::Cut(at) => format!("cut at byte {at}"),
            Change::Remove { start, end } => format!("remove bytes {start}..{end}"),
            Change::Insert { at, text } => format!("insert {:?} at byte {at}", INSERTIONS[usize::from(text)]),
            Change::CopyLine { start, end } => format!("copy the line of bytes {start}..{end}"),
            Change::JoinLines(at) => format!("remove the line break at byte {at}"),
        }
    }
}

/// True for a byte of a word token.
fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 0x80
}

/// The changes of one input, in a fixed order: the cuts, the removals, the insertions, the copies of lines, and
/// the joins of lines.
struct Changes {
    /// The starts and the ends of the tokens, and 0 and the length of the input, sorted, with no duplicates.
    boundaries: Vec<u32>,
    /// The number of boundaries before the end of the input.
    cuts: usize,
    tokens: Vec<(u32, u32)>,
    /// The start and the end of each line, without the line break.
    lines: Vec<(u32, u32)>,
    /// The positions of the line breaks that another byte follows.
    joins: Vec<u32>,
}

impl Changes {
    /// Find the tokens and the lines of an input. O(n) in the bytes of the input.
    ///
    /// # Panics
    ///
    /// If the input has 4 GB or more.
    fn new(source: &[u8]) -> Self {
        let length = u32::try_from(source.len()).expect("an input has less than 4 GB");
        let mut tokens = Vec::new();
        let mut index = 0;
        while index < source.len() {
            let byte = source[index];
            if byte.is_ascii_whitespace() {
                index += 1;
                continue;
            }
            let start = index;
            index += 1;
            if is_word_byte(byte) {
                while index < source.len() && is_word_byte(source[index]) {
                    index += 1;
                }
            }
            tokens.push((start as u32, index as u32));
        }
        let mut boundaries: Vec<u32> = tokens.iter().flat_map(|&(start, end)| [start, end]).collect();
        boundaries.push(0);
        boundaries.push(length);
        boundaries.sort_unstable();
        boundaries.dedup();
        let cuts = boundaries.len() - 1;

        let mut lines = Vec::new();
        let mut joins = Vec::new();
        let mut start = 0u32;
        for (position, &byte) in source.iter().enumerate() {
            if byte == b'\n' {
                lines.push((start, position as u32));
                start = position as u32 + 1;
                if position + 1 < source.len() {
                    joins.push(position as u32);
                }
            }
        }
        if start < length {
            lines.push((start, length));
        }
        Self {
            boundaries,
            cuts,
            tokens,
            lines,
            joins,
        }
    }

    /// The number of changes.
    fn count(&self) -> usize {
        self.cuts + self.tokens.len() + INSERTIONS.len() * self.boundaries.len() + self.lines.len() + self.joins.len()
    }

    /// The change with an index less than `count`.
    fn get(&self, mut index: usize) -> Change {
        if index < self.cuts {
            return Change::Cut(self.boundaries[index]);
        }
        index -= self.cuts;
        if index < self.tokens.len() {
            let (start, end) = self.tokens[index];
            return Change::Remove { start, end };
        }
        index -= self.tokens.len();
        let insertions = INSERTIONS.len() * self.boundaries.len();
        if index < insertions {
            return Change::Insert {
                at: self.boundaries[index / INSERTIONS.len()],
                text: (index % INSERTIONS.len()) as u8,
            };
        }
        index -= insertions;
        if index < self.lines.len() {
            let (start, end) = self.lines[index];
            return Change::CopyLine { start, end };
        }
        Change::JoinLines(self.joins[index - self.lines.len()])
    }
}

/// The SplitMix64 generator. The same seed gives the same numbers in each build.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    /// A number from 0 to `bound`, `bound` included. The small bias of the modulo does not matter here.
    fn up_to(&mut self, bound: usize) -> usize {
        (self.next() % (bound as u64 + 1)) as usize
    }
}

/// The FNV-1a hash of a name, to give each input its own sequence of random numbers.
fn name_hash(name: &str) -> u64 {
    name.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The indices of the changes to parse, sorted: all indices when `count` is not more than `limit`, and otherwise
/// `limit` different indices that the generator selects (the algorithm of Floyd). O(limit).
fn select(count: usize, limit: usize, random: &mut Random) -> Vec<usize> {
    if count <= limit {
        return (0..count).collect();
    }
    let mut selected = HashSet::with_capacity(limit);
    for last in count - limit..count {
        let candidate = random.up_to(last);
        if !selected.insert(candidate) {
            selected.insert(last);
        }
    }
    let mut indices: Vec<usize> = selected.into_iter().collect();
    indices.sort_unstable();
    indices
}

/// The kind of a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailureKind {
    /// The child gave no reply in the time limit, and a new child gave no reply in `RETRY_FACTOR` times the limit.
    Timeout,
    /// The child stopped at a failed assertion.
    Assertion,
    /// The child stopped for a different reason, for example a signal.
    Crash,
    /// The tree has a node with an incorrect range.
    Tree,
    /// The child gave no reply in the time limit, and a new child gave a reply in `RETRY_FACTOR` times the limit.
    /// A slow parse is in `failures.tsv`, but it does not make the run fail. On a shared machine, the parse of a
    /// large file can take more than the limit.
    Slow,
}

impl FailureKind {
    const ALL: [FailureKind; 5] = [
        FailureKind::Timeout,
        FailureKind::Assertion,
        FailureKind::Crash,
        FailureKind::Tree,
        FailureKind::Slow,
    ];

    fn name(self) -> &'static str {
        match self {
            FailureKind::Timeout => "timeout",
            FailureKind::Assertion => "assertion",
            FailureKind::Crash => "crash",
            FailureKind::Tree => "tree",
            FailureKind::Slow => "slow",
        }
    }
}

/// A text for the exit status of a child: its exit code or its signal.
fn describe_status(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(signal) = status.signal() {
            return format!("signal {signal}");
        }
    }
    status
        .code()
        .map_or_else(|| status.to_string(), |code| format!("exit code {code}"))
}

/// A failed parse of a changed input.
struct Failure {
    input: usize,
    /// The index of the change in the changes of the input.
    index: usize,
    change: Change,
    kind: FailureKind,
    message: String,
    bytes: Vec<u8>,
}

/// The reply of a child for one input.
enum Reply {
    Ok,
    /// The tree has an incorrect range. The text tells the node.
    Tree(String),
    Timeout,
    /// The child stopped. The text is the end of its standard error.
    Stopped(ExitStatus, String),
}

/// A child process that parses the inputs that the parent sends.
struct Worker {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<(u8, String)>,
    stderr: Arc<Mutex<Vec<u8>>>,
    threads: Vec<JoinHandle<()>>,
}

impl Worker {
    /// Start a child process with the hidden task `fuzz-worker`. The shell sets the size limit of a core file to
    /// 0, so that a failed assertion does not write a core file. It also sets the limit of the virtual memory to
    /// `MEMORY_LIMIT_KB`. A parse that grows with no end then stops with the allocation failure of the runtime,
    /// before it takes the memory of a shared machine.
    fn spawn(executable: &Path) -> io::Result<Worker> {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "ulimit -c 0 && ulimit -v {MEMORY_LIMIT_KB} && exec \"$0\" fuzz-worker"
            ))
            .arg(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().expect("the stdin of the child is a pipe");
        let mut stdout = child.stdout.take().expect("the stdout of the child is a pipe");
        let mut stderr_pipe = child.stderr.take().expect("the stderr of the child is a pipe");
        let (sender, replies) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut header = [0u8; 5];
            while stdout.read_exact(&mut header).is_ok() {
                let length = u32::from_le_bytes([header[1], header[2], header[3], header[4]]) as usize;
                let mut message = vec![0u8; length];
                if stdout.read_exact(&mut message).is_err()
                    || sender
                        .send((header[0], String::from_utf8_lossy(&message).into_owned()))
                        .is_err()
                {
                    return;
                }
            }
        });
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let tail = Arc::clone(&stderr);
        let stderr_reader = thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            while let Ok(count) = stderr_pipe.read(&mut buffer) {
                if count == 0 {
                    return;
                }
                let mut kept = tail.lock().expect("the stderr buffer is not poisoned");
                kept.extend_from_slice(&buffer[..count]);
                if kept.len() > STDERR_TAIL {
                    let excess = kept.len() - STDERR_TAIL;
                    kept.drain(..excess);
                }
            }
        });
        Ok(Worker {
            child,
            stdin,
            replies,
            stderr,
            threads: vec![reader, stderr_reader],
        })
    }

    /// Close the input of the child, wait for the end of the child and of the reader threads, and give the last
    /// line of the standard error of the child.
    fn collect(self) -> (ExitStatus, String) {
        let Worker {
            mut child,
            stdin,
            replies,
            stderr,
            threads,
        } = self;
        drop(stdin);
        let status = child.wait().unwrap_or_default();
        drop(replies);
        for thread in threads {
            let _ = thread.join();
        }
        let stderr = stderr.lock().expect("the stderr buffer is not poisoned");
        let text = String::from_utf8_lossy(&stderr);
        let line = text
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default();
        (status, line.trim().to_owned())
    }

    /// Send an input to the child and wait for its reply. After `Timeout` and `Stopped`, the worker is gone.
    fn parse(mut self, bytes: &[u8], limit: Duration) -> (Reply, Option<Worker>) {
        let length = u32::try_from(bytes.len())
            .expect("an input has less than 4 GB")
            .to_le_bytes();
        let sent = self
            .stdin
            .write_all(&length)
            .and_then(|()| self.stdin.write_all(bytes))
            .and_then(|()| self.stdin.flush());
        if sent.is_ok() {
            match self.replies.recv_timeout(limit) {
                Ok((REPLY_OK, _)) => return (Reply::Ok, Some(self)),
                Ok((_, message)) => return (Reply::Tree(message), Some(self)),
                Err(RecvTimeoutError::Timeout) => {
                    let _ = self.child.kill();
                    let _ = self.collect();
                    return (Reply::Timeout, None);
                }
                Err(RecvTimeoutError::Disconnected) => {}
            }
        }
        let (status, line) = self.collect();
        (Reply::Stopped(status, line), None)
    }

    /// Close the input of the child, and wait for its end.
    fn finish(self) {
        let _ = self.collect();
    }
}

/// The state that the threads of the parent share.
struct Run {
    inputs: Vec<Input>,
    seed: u64,
    limit: Duration,
    executable: PathBuf,
    next_input: AtomicUsize,
    parsed: AtomicUsize,
    failures: Mutex<Vec<Failure>>,
}

impl Run {
    /// Parse the bytes with the worker. Start a new worker when there is none.
    fn attempt(&self, worker: &mut Option<Worker>, bytes: &[u8], limit: Duration) -> io::Result<Reply> {
        let current = match worker.take() {
            Some(current) => current,
            None => Worker::spawn(&self.executable)?,
        };
        let (reply, next) = current.parse(bytes, limit);
        *worker = next;
        Ok(reply)
    }

    /// Parse the changes of the inputs with one child process, until no input is left.
    fn work(&self) -> io::Result<()> {
        let mut worker: Option<Worker> = None;
        let seconds = self.limit.as_secs_f64();
        loop {
            let index = self.next_input.fetch_add(1, Ordering::Relaxed);
            let Some(input) = self.inputs.get(index) else { break };
            let changes = Changes::new(&input.bytes);
            let mut random = Random(self.seed ^ name_hash(&input.name));
            let mut timeouts = 0;
            for selected in select(changes.count(), input.limit, &mut random) {
                let change = changes.get(selected);
                let bytes = change.apply(&input.bytes);
                let mut reply = self.attempt(&mut worker, &bytes, self.limit)?;
                // After some timeouts of the same input, a scan with no end is the probable cause, and a second
                // parse of each change would only make the run longer.
                let retry = matches!(reply, Reply::Timeout) && timeouts < MAX_RETRIED_TIMEOUTS;
                if retry {
                    reply = self.attempt(&mut worker, &bytes, self.limit * RETRY_FACTOR)?;
                }
                let retry_seconds = seconds * f64::from(RETRY_FACTOR);
                let failure = match reply {
                    Reply::Ok if retry => Some((
                        FailureKind::Slow,
                        format!("no reply in {seconds} s, and a reply in {retry_seconds} s"),
                    )),
                    Reply::Ok => None,
                    Reply::Tree(message) => Some((FailureKind::Tree, message)),
                    Reply::Timeout => {
                        timeouts += 1;
                        let message = if retry {
                            format!("no reply in {seconds} s, and no reply in {retry_seconds} s")
                        } else {
                            format!("no reply in {seconds} s")
                        };
                        Some((FailureKind::Timeout, message))
                    }
                    Reply::Stopped(status, line) => {
                        let kind = if line.contains("Assertion") {
                            FailureKind::Assertion
                        } else {
                            FailureKind::Crash
                        };
                        Some((kind, format!("{}: {line}", describe_status(status))))
                    }
                };
                if let Some((kind, message)) = failure {
                    self.failures
                        .lock()
                        .expect("the failure list is not poisoned")
                        .push(Failure {
                            input: index,
                            index: selected,
                            change,
                            kind,
                            message,
                            bytes,
                        });
                }
                let parsed = self.parsed.fetch_add(1, Ordering::Relaxed) + 1;
                if parsed.is_multiple_of(500_000) {
                    eprintln!("{parsed} changes");
                }
            }
        }
        if let Some(worker) = worker {
            worker.finish();
        }
        Ok(())
    }
}

/// The options of `xtask fuzz`.
struct Options {
    seconds: u64,
    seed: u64,
    jobs: usize,
    list: Option<PathBuf>,
    root: Option<PathBuf>,
    file_changes: usize,
    out: PathBuf,
}

/// Read the options of `xtask fuzz`.
fn options(args: &[String]) -> Result<Options, Box<dyn Error>> {
    let default_jobs = thread::available_parallelism()
        .map_or(1, |count| count.get())
        .min(MAX_DEFAULT_JOBS);
    let mut options = Options {
        seconds: DEFAULT_SECONDS,
        seed: DEFAULT_SEED,
        jobs: default_jobs,
        list: None,
        root: None,
        file_changes: DEFAULT_FILE_CHANGES,
        out: PathBuf::new(),
    };
    let mut out = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| iter.next().ok_or_else(|| format!("{name} needs a value\n{USAGE}"));
        match arg.as_str() {
            "--seconds" => options.seconds = value("--seconds")?.parse()?,
            "--seed" => options.seed = value("--seed")?.parse()?,
            "--jobs" => options.jobs = value("--jobs")?.parse()?,
            "--list" => options.list = Some(PathBuf::from(value("--list")?)),
            "--root" => options.root = Some(PathBuf::from(value("--root")?)),
            "--file-changes" => options.file_changes = value("--file-changes")?.parse()?,
            text if !text.starts_with("--") && out.is_none() => out = Some(PathBuf::from(text)),
            _ => return Err(USAGE.into()),
        }
    }
    options.out = out.ok_or(USAGE)?;
    if options.seconds == 0 || options.jobs == 0 {
        return Err(format!("--seconds and --jobs need a value of 1 or more\n{USAGE}").into());
    }
    Ok(options)
}

/// The inputs: the snippets of `test/syntax`, the inputs of the examples of `test/corpus`, and the files of the
/// list.
fn inputs(repository: &Path, options: &Options) -> Result<Vec<Input>, Box<dyn Error>> {
    let mut inputs = Vec::new();
    let directory = repository.join("test").join("syntax");
    let mut paths: Vec<PathBuf> = fs::read_dir(&directory)
        .map_err(|e| format!("cannot read {}: {e}", directory.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    paths.retain(|path| path.extension().is_some_and(|extension| extension == "txt"));
    paths.sort();
    for path in paths {
        let content = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let file = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        for snippet in crate::syntax::snippets(&content) {
            inputs.push(Input {
                name: format!("syntax/{file}#{}", snippet.name),
                bytes: snippet.source.into_bytes(),
                limit: TEST_INPUT_CHANGES,
            });
        }
    }
    for (name, bytes) in crate::test::example_inputs(repository)? {
        inputs.push(Input {
            name: format!("corpus/{name}"),
            bytes,
            limit: TEST_INPUT_CHANGES,
        });
    }
    if let Some(list) = &options.list {
        let text =
            fs::read_to_string(list).map_err(|e| format!("cannot read the file list {}: {e}", list.display()))?;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let path = options
                .root
                .as_deref()
                .map_or_else(|| PathBuf::from(line), |root| root.join(line));
            let bytes = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            if u32::try_from(bytes.len()).is_err() {
                continue;
            }
            inputs.push(Input {
                name: line.to_owned(),
                bytes,
                limit: options.file_changes,
            });
        }
    }
    Ok(inputs)
}

/// Write `failures.tsv` and the bytes of the failed inputs, in the order of the inputs and their changes.
fn write_failures(out: &Path, inputs: &[Input], failures: &mut [Failure]) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    for entry in fs::read_dir(out)? {
        let path = entry?.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name == "failures.tsv" || (name.starts_with("failure-") && name.ends_with(".cpp")) {
            fs::remove_file(&path)?;
        }
    }
    failures.sort_by_key(|failure| (failure.input, failure.index));
    let path = out.join("failures.tsv");
    let mut sink =
        BufWriter::new(fs::File::create(&path).map_err(|e| format!("cannot create {}: {e}", path.display()))?);
    writeln!(sink, "file\tkind\tinput\tchange\tbytes\tmessage")?;
    for (number, failure) in failures.iter().enumerate() {
        let file = if number < MAX_FAILURE_FILES {
            let name = format!("failure-{:04}.cpp", number + 1);
            fs::write(out.join(&name), &failure.bytes)?;
            name
        } else {
            "-".to_owned()
        };
        let message = failure.message.replace(['\t', '\n'], " ");
        writeln!(
            sink,
            "{file}\t{}\t{}\t{}\t{}\t{message}",
            failure.kind.name(),
            inputs[failure.input].name,
            failure.change.describe(),
            failure.bytes.len()
        )?;
    }
    sink.flush()?;
    Ok(())
}

/// Run `xtask fuzz`. Return an error when an input fails.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let options = options(args)?;
    let started = Instant::now();
    let run = Run {
        inputs: inputs(repository, &options)?,
        seed: options.seed,
        limit: Duration::from_secs(options.seconds),
        executable: std::env::current_exe()?,
        next_input: AtomicUsize::new(0),
        parsed: AtomicUsize::new(0),
        failures: Mutex::new(Vec::new()),
    };
    let results: Vec<io::Result<()>> = thread::scope(|scope| {
        let handles: Vec<_> = (0..options.jobs).map(|_| scope.spawn(|| run.work())).collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a fuzz thread does not panic"))
            .collect()
    });
    for result in results {
        result.map_err(|e| format!("cannot run a child process of xtask: {e}"))?;
    }
    let Run {
        inputs,
        parsed,
        failures,
        ..
    } = run;
    let mut failures = failures.into_inner().expect("the failure list is not poisoned");
    write_failures(&options.out, &inputs, &mut failures)?;

    let mut summary = String::new();
    for kind in FailureKind::ALL {
        let count = failures.iter().filter(|failure| failure.kind == kind).count();
        write!(summary, "  {} {count}", kind.name()).expect("a write to a String does not fail");
    }
    let failed = failures
        .iter()
        .filter(|failure| failure.kind != FailureKind::Slow)
        .count();
    println!(
        "inputs {}  changes {}  failures {failed}{summary}  wall {:.0} s",
        inputs.len(),
        parsed.into_inner(),
        started.elapsed().as_secs_f64()
    );
    if failed == 0 {
        Ok(())
    } else {
        Err(format!(
            "{failed} changed inputs failed. Refer to {}.",
            options.out.join("failures.tsv").display()
        )
        .into())
    }
}

/// Check the ranges of a tree: each node ends at or after its start and at or before the end of the input, each
/// child is in its parent, and each child starts at or after the end of the child before it. Give a text that
/// tells the first node with an incorrect range. O(n) in the nodes of the tree.
fn check_ranges(tree: &Tree, length: usize) -> Result<(), String> {
    let mut cursor = tree.walk();
    // The range of each open parent, and the end of its last child.
    let mut open: Vec<(usize, usize, usize)> = Vec::new();
    loop {
        let node = cursor.node();
        let (start, end) = (node.start_byte(), node.end_byte());
        let describe = || format!("{} {start}..{end}", node.kind());
        if start > end || end > length {
            return Err(format!("the node {} is not in the input of {length} bytes", describe()));
        }
        if let Some((parent_start, parent_end, previous_end)) = open.last_mut() {
            if start < *parent_start || end > *parent_end {
                return Err(format!(
                    "the node {} is not in its parent {parent_start}..{parent_end}",
                    describe()
                ));
            }
            if start < *previous_end {
                return Err(format!(
                    "the node {} starts before the end {previous_end} of the node before it",
                    describe()
                ));
            }
            *previous_end = end;
        }
        if cursor.goto_first_child() {
            open.push((start, end, start));
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Ok(());
            }
            open.pop();
        }
    }
}

/// Run the hidden task `fuzz-worker`: read inputs from the standard input, parse each one, and write a reply.
///
/// An input is its length as 4 little-endian bytes and its bytes. A reply is one byte, `REPLY_OK` or
/// `REPLY_TREE`, the length of a message as 4 little-endian bytes, and the message.
pub fn worker() -> Result<(), Box<dyn Error>> {
    let language = Language::new(tree_sitter_cpp::LANGUAGE);
    let mut parser = new_parser(&language);
    let mut input = io::stdin().lock();
    let mut output = BufWriter::new(io::stdout().lock());
    let mut source = Vec::new();
    loop {
        let mut length = [0u8; 4];
        match input.read_exact(&mut length) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        source.resize(u32::from_le_bytes(length) as usize, 0);
        input.read_exact(&mut source)?;
        let result = match parser.parse(&source, None) {
            Some(tree) => check_ranges(&tree, source.len()),
            None => Err("the parser gave no tree".to_owned()),
        };
        let (kind, message) = match result {
            Ok(()) => (REPLY_OK, String::new()),
            Err(message) => (REPLY_TREE, message),
        };
        output.write_all(&[kind])?;
        output.write_all(&u32::try_from(message.len()).unwrap_or(u32::MAX).to_le_bytes())?;
        output.write_all(message.as_bytes())?;
        output.flush()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tokens_give_each_kind_of_change() {
        let source = b"int x;\ny = 1;\n";
        let changes = Changes::new(source);
        assert_eq!(
            changes.tokens,
            vec![(0, 3), (4, 5), (5, 6), (7, 8), (9, 10), (11, 12), (12, 13)]
        );
        assert_eq!(changes.boundaries, vec![0, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
        assert_eq!(changes.lines, vec![(0, 6), (7, 13)]);
        assert_eq!(changes.joins, vec![6]);
        assert_eq!(changes.count(), 12 + 7 + 18 * 13 + 2 + 1);
        let all: Vec<Vec<u8>> = (0..changes.count())
            .map(|index| changes.get(index).apply(source))
            .collect();
        assert!(all.contains(&b"int x".to_vec()));
        assert!(all.contains(&b"int ;\ny = 1;\n".to_vec()));
        assert!(all.contains(&b"int x;\ny }= 1;\n".to_vec()));
        assert!(all.contains(&b"int x;\nint x;\ny = 1;\n".to_vec()));
        assert!(all.contains(&b"int x;y = 1;\n".to_vec()));
        assert!(all.contains(&b"int x;\ny = 1;\nR\"(".to_vec()));
    }

    #[test]
    fn a_line_with_no_line_break_at_the_end_is_a_line() {
        let changes = Changes::new(b"a\nb");
        assert_eq!(changes.lines, vec![(0, 1), (2, 3)]);
        assert_eq!(Change::CopyLine { start: 2, end: 3 }.apply(b"a\nb"), b"a\nb\nb");
        assert_eq!(Changes::new(b"").count(), INSERTIONS.len());
    }

    #[test]
    fn the_selection_has_the_limit_and_the_same_seed_gives_the_same_changes() {
        let selection = select(100_000, 32, &mut Random(7));
        assert_eq!(selection.len(), 32);
        assert!(selection.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(selection.iter().all(|&index| index < 100_000));
        assert_eq!(selection, select(100_000, 32, &mut Random(7)));
        assert_ne!(selection, select(100_000, 32, &mut Random(8)));
        assert_eq!(select(5, 32, &mut Random(7)), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn the_ranges_of_a_tree_with_errors_are_correct() {
        let language = Language::new(tree_sitter_cpp::LANGUAGE);
        let mut parser = new_parser(&language);
        for source in [
            "int x = f(1);\n",
            "void f() {\n  switch (x) {\n  case 1:\n",
            "typedef void (WINAPI *}\nint x;\n",
        ] {
            let tree = parser
                .parse(source, None)
                .expect("a parse with no time limit gives a tree");
            assert_eq!(check_ranges(&tree, source.len()), Ok(()), "{source}");
        }
    }

    /// A shell script in its own temporary directory, in the place of xtask as a child. The directory is removed at
    /// the end of the test.
    struct Script(PathBuf);

    impl Script {
        fn new(name: &str, body: &str) -> Script {
            use std::os::unix::fs::PermissionsExt as _;
            let directory = std::env::temp_dir().join(format!("xtask-fuzz-{}-{name}", std::process::id()));
            fs::create_dir_all(&directory).expect("the temporary directory is writable");
            let path = directory.join("worker");
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("the script is writable");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("the script can be executable");
            Script(directory)
        }

        fn path(&self) -> PathBuf {
            self.0.join("worker")
        }
    }

    impl Drop for Script {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The frame of "int x;" has 10 bytes: the length and the text.
    const INPUT: &[u8] = b"int x;";

    #[test]
    fn a_child_that_does_not_reply_stops_at_the_time_limit() {
        let script = Script::new("timeout", "exec sleep 30");
        let worker = Worker::spawn(&script.path()).expect("the script starts");
        let started = Instant::now();
        let (reply, next) = worker.parse(INPUT, Duration::from_millis(200));
        assert!(matches!(reply, Reply::Timeout));
        assert!(next.is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_child_that_stops_at_an_assertion_gives_the_line_of_the_assertion() {
        let script = Script::new(
            "assertion",
            "head -c 10 > /dev/null\necho \"xtask: src/scanner.c:1: f: Assertion \\`x' failed.\" >&2\nkill -ABRT $$",
        );
        let worker = Worker::spawn(&script.path()).expect("the script starts");
        let (reply, next) = worker.parse(INPUT, Duration::from_secs(20));
        let Reply::Stopped(status, line) = reply else {
            panic!("the child did not stop");
        };
        assert_eq!(describe_status(status), "signal 6");
        assert_eq!(line, "xtask: src/scanner.c:1: f: Assertion `x' failed.");
        assert!(next.is_none());
    }

    #[test]
    fn a_child_that_replies_parses_the_next_input() {
        let script = Script::new(
            "reply",
            "head -c 10 > /dev/null\nprintf '\\000\\000\\000\\000\\000'\nhead -c 10 > /dev/null\n\
             printf '\\001\\003\\000\\000\\000bad'\nexec cat > /dev/null",
        );
        let worker = Worker::spawn(&script.path()).expect("the script starts");
        let (reply, next) = worker.parse(INPUT, Duration::from_secs(20));
        assert!(matches!(reply, Reply::Ok));
        let (reply, next) = next.expect("the child stays").parse(INPUT, Duration::from_secs(20));
        assert!(matches!(reply, Reply::Tree(message) if message == "bad"));
        next.expect("the child stays").finish();
    }
}
