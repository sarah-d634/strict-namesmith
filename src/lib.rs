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
//!
//! Both of those pick whole entries out of a list you supply. If you'd
//! rather generate names that merely resemble a set of examples - useful
//! when you don't have enough entries to make picking from a list feel
//! varied - train a [`MarkovGenerator`] on them instead:
//!
//! ```
//! use namesmith::{MarkovGenerator, Rng};
//!
//! let examples = vec!["Aria".to_string(), "Ariana".to_string(), "Marina".to_string()];
//! let mut generator = MarkovGenerator::new(examples, 2, Rng::from_seed(1)).unwrap();
//! assert!(!generator.generate().is_empty());
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
    /// hyphen, or apostrophe, a duplicate entry, or (for a weighted list) an
    /// entry with a weight of zero. This is the default.
    #[default]
    Strict,
    /// Trim whitespace, drop empty entries, drop duplicates, and (for a
    /// weighted list) drop entries with a weight of zero, instead of
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
    /// An entry in a weighted list had a weight of zero, so it could never
    /// be picked.
    ZeroWeight { list: String, entry: String },
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
            BuildError::ZeroWeight { list, entry } => {
                write!(f, "{list} list entry {entry:?} has a weight of zero")
            }
        }
    }
}

impl std::error::Error for BuildError {}

/// Generates "First Last" names by picking one entry from a first-name list
/// and one from a last-name list.
pub struct NameGenerator {
    first_names: Vec<(String, u32)>,
    last_names: Vec<(String, u32)>,
    rng: Rng,
}

impl NameGenerator {
    /// Builds a generator, validating both lists under [`Strictness::Strict`].
    /// Every entry is equally likely to be picked; use [`NameGenerator::new_weighted`]
    /// if some names should come up more often than others.
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
        Self::with_strictness_weighted(
            to_weighted(first_names),
            to_weighted(last_names),
            rng,
            strictness,
        )
    }

    /// Builds a generator from weighted lists, validating both under
    /// [`Strictness::Strict`]. An entry's weight controls how often it's
    /// picked relative to the other entries in its list: a weight of 2 comes
    /// up twice as often as a weight of 1.
    pub fn new_weighted(
        first_names: Vec<(String, u32)>,
        last_names: Vec<(String, u32)>,
        rng: Rng,
    ) -> Result<Self, BuildError> {
        Self::with_strictness_weighted(first_names, last_names, rng, Strictness::Strict)
    }

    /// Builds a generator from weighted lists, validating both under the
    /// given [`Strictness`].
    pub fn with_strictness_weighted(
        first_names: Vec<(String, u32)>,
        last_names: Vec<(String, u32)>,
        rng: Rng,
        strictness: Strictness,
    ) -> Result<Self, BuildError> {
        let first_names = validate_list("first_names", first_names, strictness)?;
        let last_names = validate_list("last_names", last_names, strictness)?;
        Ok(NameGenerator { first_names, last_names, rng })
    }

    /// Builds a generator from raw text, one entry per line, validating
    /// both lists under [`Strictness::Strict`]. See [`parse_word_list`] for
    /// how lines are turned into entries - this is a shortcut for calling
    /// it yourself and passing the result to [`NameGenerator::new`].
    pub fn from_lines(first_names: &str, last_names: &str, rng: Rng) -> Result<Self, BuildError> {
        Self::with_strictness_from_lines(first_names, last_names, rng, Strictness::Strict)
    }

    /// Builds a generator from raw text, one entry per line, validating
    /// both lists under the given [`Strictness`]. See [`parse_word_list`]
    /// for how lines are turned into entries.
    pub fn with_strictness_from_lines(
        first_names: &str,
        last_names: &str,
        rng: Rng,
        strictness: Strictness,
    ) -> Result<Self, BuildError> {
        Self::with_strictness(
            parse_word_list(first_names),
            parse_word_list(last_names),
            rng,
            strictness,
        )
    }

    /// Picks a random first name and last name, weighted by their configured
    /// weights, and joins them with a space.
    pub fn generate(&mut self) -> String {
        let first = pick_weighted(&mut self.rng, &self.first_names);
        let last = pick_weighted(&mut self.rng, &self.last_names);
        format!("{first} {last}")
    }
}

/// Pairs every entry with a weight of 1, so an unweighted list runs through
/// the same validation and selection code as a weighted one.
fn to_weighted(entries: Vec<String>) -> Vec<(String, u32)> {
    entries.into_iter().map(|entry| (entry, 1)).collect()
}

/// Splits raw text into a word list, one entry per line. Blank lines and
/// lines whose first non-whitespace character is `#` are skipped; every
/// other line is trimmed before being kept. This is meant for loading a
/// list straight from a text file's contents without writing a parser for
/// it first - the result still goes through a generator's usual
/// [`Strictness`] check, so a typo on a line is caught the same way it
/// would be for a hand-built `Vec<String>`.
///
/// ```
/// use namesmith::parse_word_list;
///
/// let names = parse_word_list("Ada\n\n# first names\nGrace  \n");
/// assert_eq!(names, vec!["Ada".to_string(), "Grace".to_string()]);
/// ```
pub fn parse_word_list(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn validate_list(
    list: &str,
    entries: Vec<(String, u32)>,
    strictness: Strictness,
) -> Result<Vec<(String, u32)>, BuildError> {
    let entries = match strictness {
        Strictness::Strict => validate_strict(list, entries)?,
        Strictness::Lenient => validate_lenient(entries),
    };
    if entries.is_empty() {
        return Err(BuildError::EmptyList(list.to_string()));
    }
    Ok(entries)
}

fn validate_strict(
    list: &str,
    entries: Vec<(String, u32)>,
) -> Result<Vec<(String, u32)>, BuildError> {
    let mut seen = HashSet::new();
    for (index, (entry, weight)) in entries.iter().enumerate() {
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
        if *weight == 0 {
            return Err(BuildError::ZeroWeight { list: list.to_string(), entry: entry.clone() });
        }
        if !seen.insert(entry.as_str()) {
            return Err(BuildError::DuplicateEntry { list: list.to_string(), entry: entry.clone() });
        }
    }
    Ok(entries)
}

fn validate_lenient(entries: Vec<(String, u32)>) -> Vec<(String, u32)> {
    let mut seen = HashSet::new();
    let mut cleaned = Vec::with_capacity(entries.len());
    for (entry, weight) in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() || weight == 0 {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            cleaned.push((trimmed.to_string(), weight));
        }
    }
    cleaned
}

fn is_name_char(c: char) -> bool {
    c.is_alphabetic() || c == '-' || c == '\''
}

/// Picks one entry from a validated, non-empty weighted list. An entry with
/// weight `w` is `w` times as likely to be returned as one with weight 1.
fn pick_weighted<'a>(rng: &mut Rng, entries: &'a [(String, u32)]) -> &'a str {
    let total: u64 = entries.iter().map(|(_, weight)| *weight as u64).sum();
    let mut target = rng.below(total as usize) as u64;
    for (entry, weight) in entries {
        let weight = *weight as u64;
        if target < weight {
            return entry;
        }
        target -= weight;
    }
    unreachable!("target should fall within the total weight of a non-empty list")
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
    slots: HashMap<String, Vec<(String, u32)>>,
    pieces: Vec<Piece>,
    rng: Rng,
}

impl TemplateGenerator {
    /// Builds a generator, validating every slot's word list under
    /// [`Strictness::Strict`]. Every entry in a slot is equally likely to be
    /// picked; use [`TemplateGenerator::new_weighted`] if some entries should
    /// come up more often than others.
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
        let slots = slots
            .into_iter()
            .map(|(name, entries)| (name, to_weighted(entries)))
            .collect();
        Self::with_strictness_weighted(pattern, slots, rng, strictness)
    }

    /// Builds a generator whose slots carry weighted entries, validating
    /// under [`Strictness::Strict`]. An entry's weight controls how often
    /// it's picked relative to the other entries in its slot.
    pub fn new_weighted(
        pattern: &str,
        slots: Vec<(&str, Vec<(String, u32)>)>,
        rng: Rng,
    ) -> Result<Self, TemplateError> {
        Self::with_strictness_weighted(pattern, slots, rng, Strictness::Strict)
    }

    /// Builds a generator whose slots carry weighted entries, validating
    /// under the given [`Strictness`].
    pub fn with_strictness_weighted(
        pattern: &str,
        slots: Vec<(&str, Vec<(String, u32)>)>,
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

    /// Builds a generator whose slots come from raw text, one entry per
    /// line, validating under [`Strictness::Strict`]. See
    /// [`parse_word_list`] for how lines are turned into entries.
    pub fn from_lines(
        pattern: &str,
        slots: Vec<(&str, &str)>,
        rng: Rng,
    ) -> Result<Self, TemplateError> {
        Self::with_strictness_from_lines(pattern, slots, rng, Strictness::Strict)
    }

    /// Builds a generator whose slots come from raw text, one entry per
    /// line, validating under the given [`Strictness`]. See
    /// [`parse_word_list`] for how lines are turned into entries.
    pub fn with_strictness_from_lines(
        pattern: &str,
        slots: Vec<(&str, &str)>,
        rng: Rng,
        strictness: Strictness,
    ) -> Result<Self, TemplateError> {
        let slots = slots
            .into_iter()
            .map(|(name, text)| (name, parse_word_list(text)))
            .collect();
        Self::with_strictness(pattern, slots, rng, strictness)
    }

    /// Renders the pattern once, picking a weighted-random entry from each
    /// slot's word list.
    pub fn generate(&mut self) -> String {
        let mut out = String::new();
        for piece in &self.pieces {
            match piece {
                Piece::Literal(text) => out.push_str(text),
                Piece::Slot(name) => {
                    let list = &self.slots[name];
                    out.push_str(pick_weighted(&mut self.rng, list));
                }
            }
        }
        out
    }
}

/// Why building a [`MarkovGenerator`] failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkovError {
    /// `order` was zero; a chain needs at least one character of context to
    /// pick the next one from.
    ZeroOrder,
    /// No training examples were given.
    EmptyExamples,
    /// A training example was the empty string.
    EmptyEntry { index: usize },
    /// A training example contained a character other than a letter,
    /// hyphen, or apostrophe.
    InvalidCharacter { entry: String, ch: char },
}

impl fmt::Display for MarkovError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarkovError::ZeroOrder => write!(f, "order must be at least 1"),
            MarkovError::EmptyExamples => write!(f, "no training examples were given"),
            MarkovError::EmptyEntry { index } => write!(f, "training example {index} is empty"),
            MarkovError::InvalidCharacter { entry, ch } => write!(
                f,
                "training example {entry:?} contains disallowed character {ch:?}"
            ),
        }
    }
}

impl std::error::Error for MarkovError {}

/// Marks the fixed-length run of context that precedes the first real
/// character of a training example, so a chain has something to key off of
/// when generating the start of a name.
const BOUNDARY: char = '\u{0}';
/// Recorded as the "next character" after the last real character of a
/// training example, so generation can pick to stop instead of running to
/// `max_length` every time.
const END: char = '\u{1}';

/// Default cap on generated name length, used when a [`MarkovGenerator`] is
/// built with [`MarkovGenerator::new`] instead of
/// [`MarkovGenerator::with_max_length`]. Only matters if the chain happens
/// to cycle without ever landing on the end marker.
pub const DEFAULT_MAX_LENGTH: usize = 40;

/// Generates names by learning which characters tend to follow which from a
/// set of example names, instead of picking whole words out of a list.
///
/// Where [`NameGenerator`] and [`TemplateGenerator`] only ever reproduce
/// entries you already wrote down, `MarkovGenerator` learns the texture of
/// your examples - common letter pairs, typical endings - and produces new
/// names that share that texture without being copies of the input.
///
/// `order` is how many preceding characters the chain looks at to pick the
/// next one. Order 1 only knows "which letter follows this letter" and
/// tends to produce noise; order 2 or 3 usually reads as name-like; higher
/// orders stick closer to the training data and eventually just reproduce
/// it verbatim.
///
/// ```
/// use namesmith::{MarkovGenerator, Rng};
///
/// let examples = vec![
///     "Aria".to_string(), "Ariana".to_string(), "Marina".to_string(),
///     "Ada".to_string(), "Amara".to_string(), "Ariel".to_string(),
/// ];
/// let mut generator = MarkovGenerator::new(examples, 2, Rng::from_seed(1)).unwrap();
/// let name = generator.generate();
/// assert!(!name.is_empty());
/// ```
pub struct MarkovGenerator {
    max_length: usize,
    transitions: HashMap<Vec<char>, Vec<(char, u32)>>,
    order: usize,
    rng: Rng,
}

impl MarkovGenerator {
    /// Trains a chain of the given `order` on `examples`, capping generated
    /// names at [`DEFAULT_MAX_LENGTH`] characters.
    pub fn new(examples: Vec<String>, order: usize, rng: Rng) -> Result<Self, MarkovError> {
        Self::with_max_length(examples, order, DEFAULT_MAX_LENGTH, rng)
    }

    /// Trains a chain of the given `order` on `examples`, capping generated
    /// names at `max_length` characters. The cap only matters if the chain
    /// cycles without landing on its learned end-of-name transition.
    pub fn with_max_length(
        examples: Vec<String>,
        order: usize,
        max_length: usize,
        rng: Rng,
    ) -> Result<Self, MarkovError> {
        if order == 0 {
            return Err(MarkovError::ZeroOrder);
        }
        if examples.is_empty() {
            return Err(MarkovError::EmptyExamples);
        }
        let mut transitions: HashMap<Vec<char>, Vec<(char, u32)>> = HashMap::new();
        for (index, example) in examples.iter().enumerate() {
            if example.is_empty() {
                return Err(MarkovError::EmptyEntry { index });
            }
            if let Some(ch) = example.chars().find(|c| !is_name_char(*c)) {
                return Err(MarkovError::InvalidCharacter { entry: example.clone(), ch });
            }
            train(&mut transitions, example, order);
        }
        Ok(MarkovGenerator { max_length, transitions, order, rng })
    }

    /// Trains a chain from raw text, one example per line, capping generated
    /// names at [`DEFAULT_MAX_LENGTH`] characters. See [`parse_word_list`]
    /// for how lines are turned into examples.
    pub fn from_lines(text: &str, order: usize, rng: Rng) -> Result<Self, MarkovError> {
        Self::new(parse_word_list(text), order, rng)
    }

    /// Walks the chain from its start state, picking a weighted-random next
    /// character at each step, until it lands on the learned end-of-name
    /// transition or hits `max_length` characters.
    pub fn generate(&mut self) -> String {
        let mut context = vec![BOUNDARY; self.order];
        let mut out = String::new();
        while out.chars().count() < self.max_length {
            let choices = &self.transitions[&context];
            let next = pick_weighted_char(&mut self.rng, choices);
            if next == END {
                break;
            }
            out.push(next);
            context.remove(0);
            context.push(next);
        }
        out
    }
}

/// Feeds one training example into a chain's transition table: `order`
/// boundary characters, then the example's own characters, then the end
/// marker, so every context from "start of name" through "end of name" gets
/// recorded.
fn train(transitions: &mut HashMap<Vec<char>, Vec<(char, u32)>>, example: &str, order: usize) {
    let mut sequence: Vec<char> = vec![BOUNDARY; order];
    sequence.extend(example.chars());
    sequence.push(END);
    for window in sequence.windows(order + 1) {
        let context = window[..order].to_vec();
        let next = window[order];
        let counts = transitions.entry(context).or_default();
        match counts.iter_mut().find(|(ch, _)| *ch == next) {
            Some((_, count)) => *count += 1,
            None => counts.push((next, 1)),
        }
    }
}

/// Picks one character from a non-empty weighted list of `(char, count)`
/// pairs, the same way [`pick_weighted`] does for word list entries.
fn pick_weighted_char(rng: &mut Rng, choices: &[(char, u32)]) -> char {
    let total: u64 = choices.iter().map(|(_, count)| *count as u64).sum();
    let mut target = rng.below(total as usize) as u64;
    for (ch, count) in choices {
        let count = *count as u64;
        if target < count {
            return *ch;
        }
        target -= count;
    }
    unreachable!("target should fall within the total weight of a non-empty list")
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

    #[test]
    fn weighted_strict_rejects_zero_weight() {
        let err = NameGenerator::new_weighted(
            vec![("Ada".to_string(), 0)],
            vec![("Lovelace".to_string(), 1)],
            Rng::from_seed(0),
        )
        .unwrap_err();
        assert_eq!(
            err,
            BuildError::ZeroWeight { list: "first_names".to_string(), entry: "Ada".to_string() }
        );
    }

    #[test]
    fn weighted_lenient_drops_zero_weight_entry() {
        let generator = NameGenerator::with_strictness_weighted(
            vec![("Ada".to_string(), 0), ("Grace".to_string(), 1)],
            vec![("Lovelace".to_string(), 1)],
            Rng::from_seed(0),
            Strictness::Lenient,
        );
        assert!(generator.is_ok());
    }

    #[test]
    fn weighted_lenient_rejects_list_left_empty_by_dropped_zero_weights() {
        let err = NameGenerator::with_strictness_weighted(
            vec![("Ada".to_string(), 0)],
            vec![("Lovelace".to_string(), 1)],
            Rng::from_seed(0),
            Strictness::Lenient,
        )
        .unwrap_err();
        assert_eq!(err, BuildError::EmptyList("first_names".to_string()));
    }

    #[test]
    fn parse_word_list_skips_blank_lines_and_comments_and_trims() {
        let parsed = parse_word_list("Ada\n\n  \n# first names\nGrace  \n\t\n");
        assert_eq!(parsed, strs(&["Ada", "Grace"]));
    }

    #[test]
    fn name_generator_from_lines_builds_a_working_generator() {
        let mut generator = NameGenerator::from_lines(
            "Ada\nGrace\n",
            "# surnames\nLovelace\nHopper\n",
            Rng::from_seed(2),
        )
        .unwrap();
        let name = generator.generate();
        let mut parts = name.split(' ');
        assert!(["Ada", "Grace"].contains(&parts.next().unwrap()));
        assert!(["Lovelace", "Hopper"].contains(&parts.next().unwrap()));
    }

    #[test]
    fn name_generator_from_lines_still_validates_strictly() {
        let err = NameGenerator::from_lines("Ada\nAda\n", "Lovelace\n", Rng::from_seed(0))
            .unwrap_err();
        assert!(matches!(err, BuildError::DuplicateEntry { .. }));
    }

    #[test]
    fn template_generator_from_lines_builds_a_working_generator() {
        let mut generator = TemplateGenerator::from_lines(
            "{first} {last}",
            vec![("first", "Ada\nGrace\n"), ("last", "Lovelace\nHopper\n")],
            Rng::from_seed(5),
        )
        .unwrap();
        let name = generator.generate();
        assert_eq!(name.split(' ').count(), 2);
    }

    #[test]
    fn heavier_weight_is_picked_far_more_often() {
        let mut generator = NameGenerator::new_weighted(
            vec![("Rare".to_string(), 1), ("Common".to_string(), 99)],
            vec![("Surname".to_string(), 1)],
            Rng::from_seed(7),
        )
        .unwrap();

        let common_count =
            (0..200).filter(|_| generator.generate().starts_with("Common")).count();
        assert!(common_count > 150, "expected the heavily weighted entry to dominate, got {common_count}/200");
    }

    #[test]
    fn template_weighted_favors_heavier_slot_entry() {
        let mut generator = TemplateGenerator::new_weighted(
            "{word}",
            vec![("word", vec![("rare".to_string(), 1), ("common".to_string(), 99)])],
            Rng::from_seed(3),
        )
        .unwrap();

        let common_count = (0..200).filter(|_| generator.generate() == "common").count();
        assert!(common_count > 150, "expected the heavily weighted entry to dominate, got {common_count}/200");
    }

    #[test]
    fn markov_rejects_zero_order() {
        let err = MarkovGenerator::new(strs(&["Ada"]), 0, Rng::from_seed(0)).unwrap_err();
        assert_eq!(err, MarkovError::ZeroOrder);
    }

    #[test]
    fn markov_rejects_empty_examples() {
        let err = MarkovGenerator::new(vec![], 2, Rng::from_seed(0)).unwrap_err();
        assert_eq!(err, MarkovError::EmptyExamples);
    }

    #[test]
    fn markov_rejects_empty_entry() {
        let err = MarkovGenerator::new(strs(&["Ada", ""]), 2, Rng::from_seed(0)).unwrap_err();
        assert_eq!(err, MarkovError::EmptyEntry { index: 1 });
    }

    #[test]
    fn markov_rejects_invalid_character() {
        let err = MarkovGenerator::new(strs(&["Ada 2"]), 2, Rng::from_seed(0)).unwrap_err();
        assert!(matches!(err, MarkovError::InvalidCharacter { ch: ' ', .. }));
    }

    #[test]
    fn markov_generates_nonempty_names_within_max_length() {
        let examples = strs(&[
            "Aria", "Ariana", "Marina", "Ada", "Amara", "Ariel", "Mara", "Ana",
        ]);
        let mut generator = MarkovGenerator::new(examples, 2, Rng::from_seed(11)).unwrap();
        for _ in 0..50 {
            let name = generator.generate();
            assert!(!name.is_empty());
            assert!(name.chars().count() <= DEFAULT_MAX_LENGTH);
            assert!(name.chars().all(is_name_char));
        }
    }

    #[test]
    fn markov_same_seed_same_output() {
        let examples = || strs(&["Aria", "Ariana", "Marina", "Ada", "Amara"]);
        let mut a = MarkovGenerator::new(examples(), 2, Rng::from_seed(99)).unwrap();
        let mut b = MarkovGenerator::new(examples(), 2, Rng::from_seed(99)).unwrap();
        for _ in 0..20 {
            assert_eq!(a.generate(), b.generate());
        }
    }

    #[test]
    fn markov_respects_custom_max_length() {
        let mut generator =
            MarkovGenerator::with_max_length(strs(&["Ada", "Amara", "Ariana"]), 1, 3, Rng::from_seed(5))
                .unwrap();
        for _ in 0..50 {
            assert!(generator.generate().chars().count() <= 3);
        }
    }

    #[test]
    fn markov_from_lines_builds_a_working_generator() {
        let mut generator =
            MarkovGenerator::from_lines("Ada\nGrace\n\n# more\nAlan\n", 2, Rng::from_seed(2)).unwrap();
        assert!(!generator.generate().is_empty());
    }

    #[test]
    fn markov_order_one_can_reproduce_a_single_example() {
        let mut generator = MarkovGenerator::new(strs(&["Ada"]), 1, Rng::from_seed(0)).unwrap();
        assert_eq!(generator.generate(), "Ada");
    }
}
