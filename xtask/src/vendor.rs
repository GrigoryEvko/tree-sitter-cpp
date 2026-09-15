//! Put the repairs of the fork on a new release of a vendored tree-sitter crate.
//!
//! The fork vendors two crates and changes them in place: `vendor/tree-sitter`, the runtime, and
//! `vendor/tree-sitter-generate`, the generator of the parser tables. `vendor/upstream` holds the
//! pristine sources of the release that each copy starts from. With those sources, an update to a
//! new release is a three-way merge and not a new hand edit of each repair.
//!
//! `cargo xtask vendor --to VERSION` reads the crates of that version, merges each file, and writes
//! the result. The three inputs of each file are:
//!
//! - The base: the file of the pristine release in `vendor/upstream`
//! - One side: the file of the copy of the fork
//! - The other side: the file of the new pristine release
//!
//! The command writes the conflict markers of `git merge-file` into the copy of the fork, and it
//! prints one line for each file. A person then resolves each conflict, runs `cargo xtask generate`,
//! and runs the gate.
//!
//! `cargo xtask vendor --check` compares the pristine sources with the hashes that the last update
//! recorded. It fails when a file of `vendor/upstream` changed. A test runs this check.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const USAGE: &str = "usage: cargo xtask vendor --check | cargo xtask vendor --to VERSION";

/// The crates that the fork vendors. The two crates have the same version.
const CRATES: [&str; 2] = ["tree-sitter", "tree-sitter-generate"];

/// The directory of the pristine sources, relative to the repository.
const UPSTREAM: &str = "vendor/upstream";

/// The files that the crate registry adds and that the pristine sources do not keep.
const REGISTRY_FILES: [&str; 3] = [".cargo-ok", ".cargo_vcs_info.json", ".cargo-checksum.json"];

/// The result of the merge of one file.
enum Merged {
    /// Only the new release changed the file. The copy of the fork takes the new file.
    Upstream,
    /// Only the fork changed the file. The copy of the fork keeps its file.
    Fork,
    /// The two sides changed the file, and the merge has no conflict.
    Clean,
    /// The two sides changed the file, and the merge has this number of conflicts.
    Conflict(u32),
}

/// Write the SHA-256 of `bytes` as 64 hexadecimal digits. O(n) in the bytes.
fn sha256(bytes: &[u8]) -> String {
    /// The first 32 bits of the fractional parts of the cube roots of the first 64 primes.
    const K: [u32; 64] = [
        0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1, 0x923f_82a4,
        0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3, 0x72be_5d74, 0x80de_b1fe,
        0x9bdc_06a7, 0xc19b_f174, 0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f,
        0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da, 0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7,
        0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967, 0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc,
        0x5338_0d13, 0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b,
        0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070, 0x19a4_c116,
        0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3,
        0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208, 0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c, 0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut message = bytes.to_vec();
    let length = (bytes.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&length.to_be_bytes());

    for block in message.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, word) in block.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let a = words[index - 15];
            let b = words[index - 2];
            let s0 = a.rotate_right(7) ^ a.rotate_right(18) ^ (a >> 3);
            let s1 = b.rotate_right(17) ^ b.rotate_right(19) ^ (b >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let mut w = state;
        for (index, word) in words.iter().enumerate() {
            let s1 = w[4].rotate_right(6) ^ w[4].rotate_right(11) ^ w[4].rotate_right(25);
            let choice = (w[4] & w[5]) ^ (!w[4] & w[6]);
            let temp1 = w[7]
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(*word);
            let s0 = w[0].rotate_right(2) ^ w[0].rotate_right(13) ^ w[0].rotate_right(22);
            let majority = (w[0] & w[1]) ^ (w[0] & w[2]) ^ (w[1] & w[2]);
            let temp2 = s0.wrapping_add(majority);
            w = [
                temp1.wrapping_add(temp2),
                w[0],
                w[1],
                w[2],
                w[3].wrapping_add(temp1),
                w[4],
                w[5],
                w[6],
            ];
        }
        for (value, part) in state.iter_mut().zip(w) {
            *value = value.wrapping_add(part);
        }
    }
    state.iter().map(|value| format!("{value:08x}")).collect()
}

/// The paths of each file below `root`, relative to `root`, in the byte order of the paths.
///
/// The order is the order of `LC_ALL=C sort`, so that `sha256sum` writes the same manifest.
/// O(n log n) in the files of the tree.
fn files(root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        let entries =
            fs::read_dir(&directory).map_err(|e| format!("cannot read {}: {e}", directory.display()))?;
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                directories.push(path);
            } else {
                found.push(path.strip_prefix(root)?.to_owned());
            }
        }
    }
    found.sort_by(|a, b| a.as_os_str().as_encoded_bytes().cmp(b.as_os_str().as_encoded_bytes()));
    Ok(found)
}

/// One line for each file of `root`: the SHA-256, two spaces, and the path.
///
/// The layout is the layout of `sha256sum`. O(n) in the bytes of the tree.
fn manifest(root: &Path) -> Result<String, Box<dyn Error>> {
    let mut text = String::new();
    for path in files(root)? {
        let bytes = fs::read(root.join(&path))?;
        text.push_str(&format!("{}  {}\n", sha256(&bytes), path.display()));
    }
    Ok(text)
}

/// The path and the version of a line of a manifest: the path first.
fn split_line(line: &str) -> Option<(&str, &str)> {
    let (hash, path) = line.split_once("  ")?;
    Some((path, hash))
}

/// The value of the first `version = "..."` line of a Cargo manifest.
fn crate_version(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.split('"').next())
        .map(str::to_owned)
}

/// The directory of the pristine sources of `name`, and its version.
///
/// The directory is `vendor/upstream/NAME-VERSION`, and each crate has one directory.
fn pristine(repository: &Path, name: &str) -> Result<(PathBuf, String), Box<dyn Error>> {
    let upstream = repository.join(UPSTREAM);
    let prefix = format!("{name}-");
    let mut found = Vec::new();
    let entries = fs::read_dir(&upstream).map_err(|e| format!("cannot read {}: {e}", upstream.display()))?;
    for entry in entries {
        let path = entry?.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !path.is_dir() || !file_name.starts_with(&prefix) {
            continue;
        }
        // The name `tree-sitter-generate-0.27.0` also starts with `tree-sitter-`. A version starts
        // with a digit.
        let version = &file_name[prefix.len()..];
        if version.starts_with(|c: char| c.is_ascii_digit()) {
            found.push((path.clone(), version.to_owned()));
        }
    }
    match found.len() {
        1 => Ok(found.pop().expect("the vector has one element")),
        0 => Err(format!("{}/{prefix}VERSION does not exist", upstream.display()).into()),
        _ => Err(format!("{}/{prefix}VERSION exists more than one time", upstream.display()).into()),
    }
}

/// Compare the pristine sources with the hashes that the last update recorded.
///
/// The check fails when a file of `vendor/upstream` changed, when a file is missing or new, or when
/// the version of a vendored crate does not agree with the name of its pristine directory.
/// O(n) in the bytes of the pristine sources.
pub fn check(repository: &Path) -> Result<(), Box<dyn Error>> {
    for name in CRATES {
        let (directory, version) = pristine(repository, name)?;
        let recorded_path = repository.join(UPSTREAM).join(format!("{name}-{version}.sha256"));
        let recorded = fs::read_to_string(&recorded_path)
            .map_err(|e| format!("cannot read {}: {e}", recorded_path.display()))?;
        let found = manifest(&directory)?;
        if recorded != found {
            let old: BTreeMap<&str, &str> = recorded.lines().filter_map(split_line).collect();
            let new: BTreeMap<&str, &str> = found.lines().filter_map(split_line).collect();
            let mut differences = Vec::new();
            for (path, hash) in &old {
                match new.get(path) {
                    None => differences.push(format!("  removed  {path}")),
                    Some(other) if other != hash => differences.push(format!("  changed  {path}")),
                    Some(_) => {}
                }
            }
            for path in new.keys() {
                if !old.contains_key(path) {
                    differences.push(format!("  added    {path}"));
                }
            }
            return Err(format!(
                "{} does not agree with {}. The pristine sources must not change.\n{}",
                directory.display(),
                recorded_path.display(),
                differences.join("\n")
            )
            .into());
        }
        let vendored = repository.join("vendor").join(name).join("Cargo.toml");
        let text =
            fs::read_to_string(&vendored).map_err(|e| format!("cannot read {}: {e}", vendored.display()))?;
        let vendored_version =
            crate_version(&text).ok_or_else(|| format!("{} has no version", vendored.display()))?;
        if vendored_version != version {
            return Err(format!(
                "vendor/{name} is the version {vendored_version}, and its pristine sources are the version {version}"
            )
            .into());
        }
    }
    Ok(())
}

/// The path of the crate archive of `name` and `version`.
///
/// The function reads the cache of the crate registry first. It downloads the archive into
/// `destination` only when the cache does not have it.
fn archive(name: &str, version: &str, destination: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let file_name = format!("{name}-{version}.crate");
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    if let Some(home) = home
        && let Ok(entries) = fs::read_dir(home.join("registry").join("cache"))
    {
        for entry in entries.flatten() {
            let path = entry.path().join(&file_name);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    let path = destination.join(&file_name);
    let url = format!("https://static.crates.io/crates/{name}/{file_name}");
    let status = Command::new("curl")
        .args(["-sSfL", "-o"])
        .arg(&path)
        .arg(&url)
        .status()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if !status.success() {
        return Err(format!("curl cannot read {url}").into());
    }
    Ok(path)
}

/// Read the crate archive of `name` and `version` into `destination`, and give the directory of its
/// sources.
fn extract(name: &str, version: &str, destination: &Path) -> Result<PathBuf, Box<dyn Error>> {
    fs::create_dir_all(destination)?;
    let path = archive(name, version, destination)?;
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&path)
        .arg("-C")
        .arg(destination)
        .status()
        .map_err(|e| format!("cannot run tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar cannot read {}", path.display()).into());
    }
    let sources = destination.join(format!("{name}-{version}"));
    if !sources.is_dir() {
        return Err(format!("{} does not exist after tar", sources.display()).into());
    }
    for file in REGISTRY_FILES {
        let _ = fs::remove_file(sources.join(file));
    }
    Ok(sources)
}

/// Copy the tree at `from` into `to`. O(n) in the bytes of the tree.
fn copy_tree(from: &Path, to: &Path) -> Result<(), Box<dyn Error>> {
    for path in files(from)? {
        let target = to.join(&path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from.join(&path), &target)?;
    }
    Ok(())
}

/// Merge one file with `git merge-file`, and write the result into `target`.
///
/// The exit code of `git merge-file` is the number of conflicts, and a negative code is an error.
fn merge_file(fork: &Path, base: &Path, new: &Path, target: &Path) -> Result<Merged, Box<dyn Error>> {
    let base_bytes = fs::read(base)?;
    let fork_bytes = fs::read(fork)?;
    if base_bytes == fork_bytes {
        return Ok(Merged::Upstream);
    }
    let new_bytes = fs::read(new)?;
    if base_bytes == new_bytes {
        fs::write(target, &fork_bytes)?;
        return Ok(Merged::Fork);
    }
    let output = Command::new("git")
        .args(["merge-file", "-p", "--diff3", "-L", "fork", "-L", "base", "-L", "new"])
        .arg(fork)
        .arg(base)
        .arg(new)
        .output()
        .map_err(|e| format!("cannot run git merge-file: {e}"))?;
    let code = output.status.code().unwrap_or(-1);
    if code < 0 {
        return Err(format!(
            "git merge-file failed for {}: {}",
            target.display(),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    fs::write(target, &output.stdout)?;
    Ok(if code == 0 {
        Merged::Clean
    } else {
        Merged::Conflict(u32::try_from(code).unwrap_or(u32::MAX))
    })
}

/// A directory that the drop removes.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Put the repairs of the fork on the release `version` of each vendored crate.
///
/// The function writes the merged files into `vendor/NAME`, the new pristine sources into
/// `vendor/upstream/NAME-VERSION`, and the new hashes next to them. It removes the pristine sources
/// of the previous release. O(n) in the bytes of the crates.
pub fn sync(repository: &Path, version: &str) -> Result<(), Box<dyn Error>> {
    let scratch = Scratch(
        std::env::temp_dir().join(format!("tree-sitter-cpp-vendor-{}", std::process::id())),
    );
    fs::create_dir_all(&scratch.0)?;
    let mut conflicts = 0u32;
    for name in CRATES {
        let (base_directory, base_version) = pristine(repository, name)?;
        if base_version == version {
            println!("{name}: the pristine sources are already the version {version}");
            continue;
        }
        println!("{name}: {base_version} -> {version}");
        let fork_directory = repository.join("vendor").join(name);
        let new_directory = extract(name, version, &scratch.0)?;
        // The merge reads the copy of the fork after the new release replaces it.
        let fork_copy = scratch.0.join(format!("{name}-fork"));
        copy_tree(&fork_directory, &fork_copy)?;

        let base_files = files(&base_directory)?;
        let fork_files = files(&fork_copy)?;
        let new_files = files(&new_directory)?;
        fs::remove_dir_all(&fork_directory)?;
        copy_tree(&new_directory, &fork_directory)?;

        for path in &base_files {
            if !fork_files.contains(path) {
                println!("  removed by the fork  {}", path.display());
                let _ = fs::remove_file(fork_directory.join(path));
                continue;
            }
            if new_files.contains(path) {
                match merge_file(
                    &fork_copy.join(path),
                    &base_directory.join(path),
                    &new_directory.join(path),
                    &fork_directory.join(path),
                )? {
                    Merged::Upstream => {}
                    Merged::Fork => println!("  fork only {}", path.display()),
                    Merged::Clean => println!("  merged    {}", path.display()),
                    Merged::Conflict(count) => {
                        println!("  CONFLICT  {}: {count} hunks", path.display());
                        conflicts += 1;
                    }
                }
            } else if fs::read(base_directory.join(path))? != fs::read(fork_copy.join(path))? {
                println!(
                    "  CONFLICT  {}: the release removed a file that the fork changed",
                    path.display()
                );
                conflicts += 1;
            }
        }
        for path in &fork_files {
            if !base_files.contains(path) && !new_files.contains(path) {
                println!("  added by the fork    {}", path.display());
                let target = fork_directory.join(path);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(fork_copy.join(path), &target)?;
            }
        }

        let upstream = repository.join(UPSTREAM);
        fs::remove_dir_all(&base_directory)?;
        let _ = fs::remove_file(upstream.join(format!("{name}-{base_version}.sha256")));
        let target = upstream.join(format!("{name}-{version}"));
        copy_tree(&new_directory, &target)?;
        fs::write(upstream.join(format!("{name}-{version}.sha256")), manifest(&target)?)?;
    }
    if conflicts > 0 {
        return Err(format!(
            "{conflicts} files have a conflict. Resolve each one, then run `cargo xtask generate` and the gate."
        )
        .into());
    }
    println!("no conflict. Run `cargo xtask generate` and the gate.");
    Ok(())
}

/// Run the vendor task.
pub fn run(repository: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    match args {
        [flag] if flag == "--check" => check(repository),
        [flag, version] if flag == "--to" => sync(repository, version),
        _ => Err(USAGE.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{check, crate_version, sha256};

    /// The directory of the repository.
    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the xtask crate is in a directory of the repository")
            .to_owned()
    }

    /// The test vectors of FIPS 180-4, and the hash of no bytes.
    #[test]
    fn sha256_agrees_with_the_published_test_vectors() {
        assert_eq!(sha256(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            sha256(&b"a".repeat(1_000_000)),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn the_version_of_a_cargo_manifest_is_its_first_version_line() {
        assert_eq!(
            crate_version("[package]\nname = \"x\"\nversion = \"0.27.0\"\n"),
            Some("0.27.0".to_owned())
        );
        assert_eq!(crate_version("[package]\nname = \"x\"\n"), None);
    }

    /// The pristine sources in vendor/upstream agree with the hashes of the last update.
    #[test]
    fn the_pristine_sources_of_the_vendored_crates_do_not_change() {
        check(&repository()).expect("the pristine sources agree with their hashes");
    }
}
