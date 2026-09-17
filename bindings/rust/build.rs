/// The table of the CRC-32 of each byte value, for the polynomial of IEEE 802.3.
fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (index, slot) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            let mask = (value & 1).wrapping_neg();
            value = (value >> 1) ^ (0xEDB8_8320 & mask);
        }
        *slot = value;
    }
    table
}

/// Add the bytes to a running CRC-32. The value starts at `0xFFFF_FFFF` and ends with a complement.
fn crc_bytes(crc: u32, bytes: &[u8], table: &[u32; 256]) -> u32 {
    let mut value = crc;
    for byte in bytes {
        value = (value >> 8) ^ table[((value ^ u32::from(*byte)) & 0xFF) as usize];
    }
    value
}

/// The identity of one file: the CRC-32 of its bytes and its length, as `CRC:LENGTH`.
fn file_identity(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let crc = crc_bytes(0xFFFF_FFFF, &bytes, &crc_table());
    format!("{:08x}:{}", !crc, bytes.len())
}

/// Every file under one directory, as pairs of the relative path and the full path, sorted by the
/// relative path. The complexity is O(n log n) in the number of files.
fn files_of(root: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_owned()];
    while let Some(directory) = stack.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("the directory entry must be readable").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("each file is under the root")
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push((relative, path));
            }
        }
    }
    files.sort();
    files
}

/// The identity of a directory of sources: the CRC-32 over the relative path and the bytes of each
/// file, and the number of files, as `CRC:COUNT`.
///
/// THE IDENTITY OF A BUILD MUST DEPEND ON EVERY SOURCE THAT THE BUILD READS. A script that listed
/// `src/parser.c` and `src/scanner.c` only left the library of the version before a change of
/// `src/seed.h`, and the reader then refused every seed with a message that named two trees that are
/// in fact one. The walk also gives cargo the list of the files to watch, so no new header can fall
/// out of it.
fn directory_identity(root: &std::path::Path) -> String {
    let table = crc_table();
    let mut crc = 0xFFFF_FFFFu32;
    let files = files_of(root);
    for (relative, path) in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        crc = crc_bytes(crc, relative.as_bytes(), &table);
        crc = crc_bytes(crc, b"\0", &table);
        crc = crc_bytes(crc, &bytes, &table);
    }
    format!("{:08x}:{}", !crc, files.len())
}

fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);

    #[cfg(target_env = "msvc")]
    c_config.flag("-utf-8");

    let parser_path = src_dir.join("parser.c");
    c_config.file(&parser_path);

    let scanner_path = src_dir.join("scanner.c");
    c_config.file(&scanner_path);

    // THE IDENTITY OF THE PARSER THAT THIS BUILD LINKS, AND THE LIST OF THE FILES THAT CARGO WATCHES.
    // `cargo build --release` in the tree builds the root package only, so a binary of another
    // package keeps the parser of its last link. A scan with that binary gives whole tables with no
    // error, and it measures a different commit. Each measurement compares these values with the
    // files of the tree that it means to measure (bin/identity.py).
    //
    // `directory_identity` writes one `rerun-if-changed` line for each file under src, so a change
    // of src/seed.h or of a header of src/tree_sitter builds the library again. A script that named
    // the two `.c` files only kept the library of the version before such a change.
    println!("cargo:rustc-env=TS_CPP_SRC_IDENTITY={}", directory_identity(src_dir));
    println!("cargo:rustc-env=TS_CPP_PARSER_C_IDENTITY={}", file_identity(&parser_path));
    println!("cargo:rustc-env=TS_CPP_SCANNER_C_IDENTITY={}", file_identity(&scanner_path));

    c_config.compile("tree-sitter-cpp");
}
