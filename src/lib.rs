//! Random names assembled from your own word lists.
//!
//! A hand-edited or scraped word list almost always has a typo in it
//! somewhere: a trailing space that quietly doubles an entry's odds, a blank
//! line that becomes an empty name, a stray comma pasted in from a
//! spreadsheet. [`NameGenerator`] refuses to build from a list like that by
//! default, and tells you exactly which entry is the problem. If you know
//! your input is messy but harmless, opt into [`Strictness::Lenient`] and
//! it will clean the list up instead of erroring.
//!
//! ```
//! use namesmith::{NameGenerator, Rng};
//!
//! let first = vec!["Ada".to_string(), "Grace".to_string()];
//! let last = vec!["Lovelace".to_string(), "Hopper".to_string()];
//!
//! let mut generator = NameGenerator::new(first, last, Rng::from_seed(1)).unwrap();
//! let name = generator.generate();
//! assert!(name.contains(' '));
//! ```

mod rng;

use std::collections::HashSet;
use std::fmt;

pub use rng::Rng;

/// Controls how strictly input word lists are validated when building a
/// [`NameGenerator`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strictness {
    /// Reject a list containing an empty entry, an entry with leading or
    /// trailing whitespace, an entry with a character other than a letter,
    /// hyphen, or apostrophe, or a duplicate entry. This is the default.
    #[default]
    Strict,
    /// Trim whitespace, drop empty entries, and drop duplicates instead of
    /// erroring. Characters outside the strict allow-list are kept as-is.
    Lenient,
}

/// Why building a [`NameGenerator`] failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// A list had no usable entries left after validation.
    EmptyList(&'static str),
    /// An entry was the empty string.
    EmptyEntry { list: &'static str, index: usize },
    /// An entry had leading or trailing whitespace.
    UntrimmedEntry { list: &'static str, entry: String },
    /// An entry contained a character outside `[A-Za-z-']`.
    InvalidCharacter { list: &'static str, entry: String, ch: char },
    /// The same entry appeared more than once in a list.
    DuplicateEntry { list: &'static str, entry: String },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::EmptyList(list) => write!(f, "{list} list has no usable entries"),
            BuildError::EmptyEntry { list, index } => {
                write!(f, "{list} list entry {index} is empty")
            }
            BuildError::UntrimmedEntry { list, entry } => write!(
                f,
                "{list} list entry {entry:?} has leading or trailing whitespace"
            ),
            BuildError::InvalidCharacter { list, entry, ch } => write!(
                f,
                "{list} list entry {entry:?} contains disallowed character {ch:?}"
            ),
            BuildError::DuplicateEntry { list, entry } => {
                write!(f, "{list} list has duplicate entry {entry:?}")
            }
        }
    }
}

impl std::error::Error for BuildError {}

/// Generates "First Last" names by picking one entry from a first-name list
/// and one from a last-name list.
pub struct NameGenerator {
    first_names: Vec<String>,
    last_names: Vec<String>,
    rng: Rng,
}

impl NameGenerator {
    /// Builds a generator, validating both lists under [`Strictness::Strict`].
    pub fn new(
        first_names: Vec<String>,
        last_names: Vec<String>,
        rng: Rng,
    ) -> Result<Self, BuildError> {
        Self::with_strictness(first_names, last_names, rng, Strictness::Strict)
    }

    /// Builds a generator, validating both lists under the given
    /// [`Strictness`].
    pub fn with_strictness(
        first_names: Vec<String>,
        last_names: Vec<String>,
        rng: Rng,
        strictness: Strictness,
    ) -> Result<Self, BuildError> {
        let first_names = validate_list("first_names", first_names, strictness)?;
        let last_names = validate_list("last_names", last_names, strictness)?;
        Ok(NameGenerator { first_names, last_names, rng })
    }

    /// Picks a random first name and last name and joins them with a space.
    pub fn generate(&mut self) -> String {
        let first = &self.first_names[self.rng.below(self.first_names.len())];
        let last = &self.last_names[self.rng.below(self.last_names.len())];
        format!("{first} {last}")
    }
}

fn validate_list(
    list: &'static str,
    entries: Vec<String>,
    strictness: Strictness,
) -> Result<Vec<String>, BuildError> {
    let entries = match strictness {
        Strictness::Strict => validate_strict(list, entries)?,
        Strictness::Lenient => validate_lenient(entries),
    };
    if entries.is_empty() {
        return Err(BuildError::EmptyList(list));
    }
    Ok(entries)
}

fn validate_strict(list: &'static str, entries: Vec<String>) -> Result<Vec<String>, BuildError> {
    let mut seen = HashSet::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.is_empty() {
            return Err(BuildError::EmptyEntry { list, index });
        }
        if entry.trim() != entry {
            return Err(BuildError::UntrimmedEntry { list, entry: entry.clone() });
        }
        if let Some(ch) = entry.chars().find(|c| !is_name_char(*c)) {
            return Err(BuildError::InvalidCharacter { list, entry: entry.clone(), ch });
        }
        if !seen.insert(entry.as_str()) {
            return Err(BuildError::DuplicateEntry { list, entry: entry.clone() });
        }
    }
    Ok(entries)
}

fn validate_lenient(entries: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut cleaned = Vec::with_capacity(entries.len());
    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            cleaned.push(trimmed.to_string());
        }
    }
    cleaned
}

fn is_name_char(c: char) -> bool {
    c.is_alphabetic() || c == '-' || c == '\''
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn strict_rejects_empty_entry() {
        let err = NameGenerator::new(strs(&["Ada", ""]), strs(&["Lovelace"]), Rng::from_seed(0))
            .unwrap_err();
        assert_eq!(err, BuildError::EmptyEntry { list: "first_names", index: 1 });
    }

    #[test]
    fn strict_rejects_untrimmed_entry() {
        let err =
            NameGenerator::new(strs(&["Ada "]), strs(&["Lovelace"]), Rng::from_seed(0))
                .unwrap_err();
        assert!(matches!(err, BuildError::UntrimmedEntry { .. }));
    }

    #[test]
    fn strict_rejects_duplicate() {
        let err = NameGenerator::new(strs(&["Ada", "Ada"]), strs(&["Lovelace"]), Rng::from_seed(0))
            .unwrap_err();
        assert!(matches!(err, BuildError::DuplicateEntry { .. }));
    }

    #[test]
    fn lenient_repairs_instead_of_erroring() {
        let generator = NameGenerator::with_strictness(
            strs(&["Ada ", "", "Ada"]),
            strs(&["Lovelace"]),
            Rng::from_seed(0),
            Strictness::Lenient,
        );
        assert!(generator.is_ok());
    }

    #[test]
    fn generate_picks_from_both_lists() {
        let mut generator = NameGenerator::new(
            strs(&["Ada", "Grace"]),
            strs(&["Lovelace", "Hopper"]),
            Rng::from_seed(123),
        )
        .unwrap();
        let name = generator.generate();
        let mut parts = name.split(' ');
        assert!(["Ada", "Grace"].contains(&parts.next().unwrap()));
        assert!(["Lovelace", "Hopper"].contains(&parts.next().unwrap()));
    }
}
