#ifndef TREE_SITTER_CPP_SEED_H_
#define TREE_SITTER_CPP_SEED_H_

#include <stdint.h>

/// THE SEED OF A PARSE: the names that a project declares as a type, as a template, or as a macro.
///
/// The scanner records the names that the FILE declares, and that record cannot hold a name from a
/// header, because no header is parsed. A seed gives the scanner the names of the whole project,
/// read one time from a file that `cargo xtask seed collect` writes. The caller owns the memory,
/// keeps it alive for as long as the parser can parse, and gives the pointer to
/// `ts_parser_set_scanner_context`. The runtime never reads the target.
///
/// THE NAMES ASCEND BY THEIR BYTES, so a reader does a binary search over them and compares the
/// bytes of the name. A hash would be smaller, and it would also make two names one entry. An
/// over-acceptance of that kind has no ERROR node and no differ row. Refer to the measurement of
/// #280.
///
/// THIS HEADER IS THE ONE DECLARATION OF THE FORMAT. The Rust reader of `xtask/src/seed.rs`
/// declares the same structs, and its test `the_longest_name_agrees_with_the_header_of_the_scanner`
/// reads `TS_CPP_SEED_WORD_SIZE` from this file and compares it with its own limit. A comment in
/// the place of that test is what let the two halves of this task disagree three times in one
/// afternoon.

/// The size of the buffer that the scanner reads a name into for a seed lookup, with the NUL. A
/// name of more than `TS_CPP_SEED_WORD_SIZE - 1` bytes is not in a seed, and the collector drops it.
///
/// THE VALUE COMES FROM A MEASUREMENT AND NOT FROM A GUESS. The collector takes 594,857 names from
/// the 64 corpus projects on 2026-09-16. A limit of 40 bytes drops 1.28% of them, and 17.61% of
/// Vulkan-Hpp, the project that loses the most. A limit of 48 drops 0.37%, and 5.34% of Vulkan-Hpp.
/// A limit of 64 drops 0.03%, which is 157 names.
///
/// ONE PROJECT STILL PAYS AT 64. stdexec loses 1.07%, 24 of its 2,236 names, because it writes a
/// diagnostic into the name of a type and its longest such name has 109 bytes. No other project
/// loses more than 0.17%.
///
/// THIS IS NOT `MACRO_WORD_SIZE` OF src/scanner.c, WHICH STAYS AT 41. That constant sizes the
/// buffer of a word that a scan compares with a LIST, and `is_region_macro_name` reads its value to
/// tell whether a word was cut. The two consumers have different requirements. One constant for
/// both would tie the seed to a number that a different code path depends on being small.
#define TS_CPP_SEED_WORD_SIZE 65

/// `TSSD` in the order of the bytes of the machine.
#define TS_CPP_SEED_MAGIC 0x44535354u

/// The version of the structs below. A reader that meets a different value gives no name.
#define TS_CPP_SEED_VERSION 1u

/// The name gives a type.
#define TS_CPP_SEED_TYPE 1u
/// The name gives a template. A template is also a type, so such an entry holds the two bits.
#define TS_CPP_SEED_TEMPLATE 2u
/// A macro defines the name. Task 276 reads this bit, and the readers of task 275 ignore it.
#define TS_CPP_SEED_MACRO 4u

/// One name of a seed.
typedef struct {
    /// The start of the name in the text block.
    uint32_t offset;
    /// The length of the name in bytes, with no NUL.
    uint16_t length;
    /// The kinds of the name, as the bits above.
    uint16_t kinds;
} TSCppSeedEntry;

/// The head of a seed. The context of the parser points at one of these.
typedef struct {
    /// `TS_CPP_SEED_MAGIC`. A reader that meets a different value gives no name.
    uint32_t magic;
    /// `TS_CPP_SEED_VERSION`.
    uint32_t version;
    /// The number of entries.
    uint32_t count;
    /// The bytes of the text block.
    uint32_t text_size;
    /// The entries, in the ascending order of the bytes of their names.
    const TSCppSeedEntry *entries;
    /// The names, one after the other, with no separator and no NUL.
    const char *text;
    /// The SHA-256 of the seed file, comments included. The tree of a file is a function of the
    /// file AND the seed, so each output that holds a tree also holds this id.
    uint8_t id[32];
} TSCppSeed;

#endif  // TREE_SITTER_CPP_SEED_H_
