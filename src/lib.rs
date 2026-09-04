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
//!
//! [`NameGenerator`] only ever produces "first last". For anything else -
//! a title, a nickname in quotes, a fantasy name built from syllables -
//! use [`TemplateGenerator`], which takes a pattern and any number of named
//! slots:
//!
//! ```
//! use namesmith::{Rng, TemplateGenerator};
//!
//! let mut generator = TemplateGenerator::new(
//!     "{first} \"{nickname}\" {last}",
//!     vec![
//!         ("first", vec!["Grace".to_string()]),
//!         ("nickname", vec!["Amazing".to_string()]),
//!         ("last", vec!["Hopper".to_string()]),
//!     ],
//!     Rng::from_seed(1),
//! )
//! .unwrap();
//! assert_eq!(generator.generate(), "Grace \"Amazing\" Hopper");
//! ```

mod rng;

use std::collections::{HashMap, HashSet};
use std::fmt;

pub use rng::Rng;

/// Controls how strictly input word lists are validated when building a
/// [`NameGenerator`] or [`TemplateGenerator`].
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

/// Why building a word list based generator failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// A list had no usable entries left after validation.
    EmptyList(String),
    /// An entry was the empty string.
    EmptyEntry { list: String, index: usize },
    /// An entry had leading or trailing whitespace.
    UntrimmedEntry { list: String, entry: String },
    /// An entry contained a character outside `[A-Za-z-']`.
    InvalidCharacter { list: String, entry: String, ch: char },
    /// The same entry appeared more than once in a list.
    DuplicateEntry { list: String, entry: String },
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
    list: &str,
    entries: Vec<String>,
    strictness: Strictness,
) -> Result<Vec<String>, BuildError> {
    let entries = match strictness {
        Strictness::Strict => validate_strict(list, entries)?,
        Strictness::Lenient => validate_lenient(entries),
    };
    if entries.is_empty() {
        return Err(BuildError::EmptyList(list.to_string()));
    }
    Ok(entries)
}

fn validate_strict(list: &str, entries: Vec<String>) -> Result<Vec<String>, BuildError> {
    let mut seen = HashSet::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.is_empty() {
            return Err(BuildError::EmptyEntry { list: list.to_string(), index });
        }
        if entry.trim() != entry {
            return Err(BuildError::UntrimmedEntry {
                list: list.to_string(),
                entry: entry.clone(),
            });
        }
        if let Some(ch) = entry.chars().find(|c| !is_name_char(*c)) {
            return Err(BuildError::InvalidCharacter {
                list: list.to_string(),
                entry: entry.clone(),
                ch,
            });
        }
        if !seen.insert(entry.as_str()) {
            return Err(BuildError::DuplicateEntry { list: list.to_string(), entry: entry.clone() });
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

/// Why building a [`TemplateGenerator`] failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// The pattern string had no content.
    EmptyPattern,
    /// A `{` was never closed with a matching `}`.
    UnterminatedPlaceholder { at: usize },
    /// A `}` appeared with no preceding `{`.
    UnmatchedClosingBrace { at: usize },
    /// A `{}` placeholder had no slot name inside it.
    EmptyPlaceholder { at: usize },
    /// The same slot name was passed to the builder more than once.
    DuplicateSlot { name: String },
    /// The pattern referenced a slot name that wasn't passed to the builder.
    UnknownSlot { name: String },
    /// One of the slot's word lists failed validation.
    List(BuildError),
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateError::EmptyPattern => write!(f, "pattern is empty"),
            TemplateError::UnterminatedPlaceholder { at } => {
                write!(f, "pattern has an unterminated '{{' at byte offset {at}")
            }
            TemplateError::UnmatchedClosingBrace { at } => {
                write!(f, "pattern has an unmatched '}}' at byte offset {at}")
            }
            TemplateError::EmptyPlaceholder { at } => {
                write!(f, "pattern has an empty {{}} placeholder at byte offset {at}")
            }
            TemplateError::DuplicateSlot { name } => {
                write!(f, "slot {name:?} was passed to the builder more than once")
            }
            TemplateError::UnknownSlot { name } => {
                write!(f, "pattern references slot {name:?}, which has no word list")
            }
            TemplateError::List(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for TemplateError {}

impl From<BuildError> for TemplateError {
    fn from(err: BuildError) -> Self {
        TemplateError::List(err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Literal(String),
    Slot(String),
}

/// Generates names from an arbitrary pattern such as `"{first} {last}"` or
/// `"{title} {first} \"{nickname}\" {last}"`, picking one entry from each
/// named slot's word list.
///
/// Where [`NameGenerator`] hardcodes a two-slot "first last" shape,
/// `TemplateGenerator` accepts any number of named slots and a pattern that
/// says how to arrange them, so callers aren't stuck with that one shape.
pub struct TemplateGenerator {
    slots: HashMap<String, Vec<String>>,
    pieces: Vec<Piece>,
    rng: Rng,
}

impl TemplateGenerator {
    /// Builds a generator, validating every slot's word list under
    /// [`Strictness::Strict`].
    pub fn new(
        pattern: &str,
        slots: Vec<(&str, Vec<String>)>,
        rng: Rng,
    ) -> Result<Self, TemplateError> {
        Self::with_strictness(pattern, slots, rng, Strictness::Strict)
    }

    /// Builds a generator, validating every slot's word list under the given
    /// [`Strictness`].
    pub fn with_strictness(
        pattern: &str,
        slots: Vec<(&str, Vec<String>)>,
        rng: Rng,
        strictness: Strictness,
    ) -> Result<Self, TemplateError> {
        if pattern.is_empty() {
            return Err(TemplateError::EmptyPattern);
        }
        let pieces = parse_pattern(pattern)?;

        let mut validated = HashMap::with_capacity(slots.len());
        for (name, entries) in slots {
            if validated.contains_key(name) {
                return Err(TemplateError::DuplicateSlot { name: name.to_string() });
            }
            let entries = validate_list(name, entries, strictness)?;
            validated.insert(name.to_string(), entries);
        }

        for piece in &pieces {
            if let Piece::Slot(name) = piece {
                if !validated.contains_key(name) {
                    return Err(TemplateError::UnknownSlot { name: name.clone() });
                }
            }
        }

        Ok(TemplateGenerator { slots: validated, pieces, rng })
    }

    /// Renders the pattern once, picking a random entry from each slot's
    /// word list.
    pub fn generate(&mut self) -> String {
        let mut out = String::new();
        for piece in &self.pieces {
            match piece {
                Piece::Literal(text) => out.push_str(text),
                Piece::Slot(name) => {
                    let list = &self.slots[name];
                    let entry = &list[self.rng.below(list.len())];
                    out.push_str(entry);
                }
            }
        }
        out
    }
}

/// Splits a pattern like `"{first} {last}"` into a sequence of literal text
/// and named slot placeholders.
fn parse_pattern(pattern: &str) -> Result<Vec<Piece>, TemplateError> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut chars = pattern.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '{' => {
                if !literal.is_empty() {
                    pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                }
                let mut name = String::new();
                let mut closed = false;
                for (_, c2) in chars.by_ref() {
                    if c2 == '}' {
                        closed = true;
                        break;
                    }
                    name.push(c2);
                }
                if !closed {
                    return Err(TemplateError::UnterminatedPlaceholder { at: i });
                }
                if name.is_empty() {
                    return Err(TemplateError::EmptyPlaceholder { at: i });
                }
                pieces.push(Piece::Slot(name));
            }
            '}' => return Err(TemplateError::UnmatchedClosingBrace { at: i }),
            _ => literal.push(c),
        }
    }
    if !literal.is_empty() {
        pieces.push(Piece::Literal(literal));
    }
    Ok(pieces)
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
        assert_eq!(
            err,
            BuildError::EmptyEntry { list: "first_names".to_string(), index: 1 }
        );
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

    #[test]
    fn template_renders_literals_and_slots() {
        let mut generator = TemplateGenerator::new(
            "{title} {first} {last}",
            vec![
                ("title", strs(&["Dr.", "Capt."])),
                ("first", strs(&["Ada", "Grace"])),
                ("last", strs(&["Lovelace", "Hopper"])),
            ],
            Rng::from_seed(9),
        )
        .unwrap();
        let name = generator.generate();
        let parts: Vec<&str> = name.split(' ').collect();
        assert_eq!(parts.len(), 3);
        assert!(["Dr.", "Capt."].contains(&parts[0]));
        assert!(["Ada", "Grace"].contains(&parts[1]));
        assert!(["Lovelace", "Hopper"].contains(&parts[2]));
    }

    #[test]
    fn template_supports_repeated_and_adjacent_slots() {
        let mut generator = TemplateGenerator::new(
            "{syllable}{syllable}",
            vec![("syllable", strs(&["ka", "mo", "ri"]))],
            Rng::from_seed(4),
        )
        .unwrap();
        let name = generator.generate();
        assert_eq!(name.len(), 4);
    }

    #[test]
    fn template_rejects_unknown_slot() {
        let err = TemplateGenerator::new(
            "{first} {last}",
            vec![("first", strs(&["Ada"]))],
            Rng::from_seed(0),
        )
        .unwrap_err();
        assert_eq!(err, TemplateError::UnknownSlot { name: "last".to_string() });
    }

    #[test]
    fn template_rejects_unterminated_placeholder() {
        let err = TemplateGenerator::new(
            "{first",
            vec![("first", strs(&["Ada"]))],
            Rng::from_seed(0),
        )
        .unwrap_err();
        assert_eq!(err, TemplateError::UnterminatedPlaceholder { at: 0 });
    }

    #[test]
    fn template_rejects_duplicate_slot() {
        let err = TemplateGenerator::new(
            "{first}",
            vec![("first", strs(&["Ada"])), ("first", strs(&["Grace"]))],
            Rng::from_seed(0),
        )
        .unwrap_err();
        assert_eq!(err, TemplateError::DuplicateSlot { name: "first".to_string() });
    }

    #[test]
    fn template_rejects_empty_pattern() {
        let err = TemplateGenerator::new("", vec![], Rng::from_seed(0)).unwrap_err();
        assert_eq!(err, TemplateError::EmptyPattern);
    }

    #[test]
    fn template_propagates_list_validation_errors() {
        let err = TemplateGenerator::new(
            "{first}",
            vec![("first", strs(&["Ada", "Ada"]))],
            Rng::from_seed(0),
        )
        .unwrap_err();
        assert!(matches!(err, TemplateError::List(BuildError::DuplicateEntry { .. })));
    }
}
