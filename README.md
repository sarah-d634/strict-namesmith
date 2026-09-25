# namesmith

A Rust library for generating random names by combining your own word
lists (first names, last names, fantasy syllables, whatever you have).

## The problem

Every project that needs fake names ends up with a `names.txt` someone
hand-edited or scraped from somewhere. Those files rot: a trailing space on
one line quietly doubles that entry's odds of being picked, a blank line
becomes an empty name, someone pastes a list in from a spreadsheet and now
half the entries have a stray comma. None of this throws an error, it just
produces slightly wrong output that nobody notices until a name like
`" Ada"` or `"Grace  Hopper"` shows up in a demo.

`namesmith` validates word lists strictly by default and refuses to build a
generator from a broken one, with an error that names the exact entry and
what's wrong with it. If you already know your input is messy but harmless
(scraped data, user-submitted lists), you can opt into
`Strictness::Lenient`, which trims and de-duplicates instead of erroring.

## Usage

```rust
use namesmith::{BuildError, NameGenerator, Rng, Strictness};

fn main() {
    let first_names = vec!["Ada".to_string(), "Grace".to_string(), "Alan".to_string()];
    let last_names = vec!["Lovelace".to_string(), "Hopper".to_string(), "Turing".to_string()];

    // Strict by default: catches empty entries, stray whitespace,
    // duplicates, and disallowed characters at build time.
    let mut generator = NameGenerator::new(first_names, last_names, Rng::from_entropy())
        .expect("word lists should be clean");

    for _ in 0..3 {
        println!("{}", generator.generate());
    }
}
```

A list with a formatting problem is rejected with a specific error instead
of silently skewing the output:

```rust
use namesmith::{BuildError, NameGenerator, Rng};

let result = NameGenerator::new(
    vec!["Ada".to_string(), "Ada ".to_string()], // trailing space
    vec!["Lovelace".to_string()],
    Rng::from_seed(1),
);

match result {
    Err(BuildError::UntrimmedEntry { list, entry }) => {
        println!("{list} has whitespace around {entry:?}");
    }
    _ => unreachable!(),
}
```

If the input is known to be messy but you want a generator anyway, opt in
explicitly:

```rust
use namesmith::{NameGenerator, Rng, Strictness};

let generator = NameGenerator::with_strictness(
    vec!["Ada ".to_string(), "".to_string(), "Ada".to_string()],
    vec!["Lovelace".to_string()],
    Rng::from_seed(1),
    Strictness::Lenient, // trims, drops the blank entry, drops the duplicate
);

assert!(generator.is_ok());
```

## Loading lists from text

Word lists usually start life as a text file, not a `Vec<String>` someone
typed into source. `from_lines` takes the raw contents of a file (or any
string with one entry per line) and does the split for you: blank lines
and lines starting with `#` are skipped, everything else is trimmed and
handed to the same strict-by-default validation as the other constructors.

```rust
use namesmith::{NameGenerator, Rng};

let first_names = "Ada\nGrace\n\n# add more later\nAlan\n";
let last_names = "Lovelace\nHopper\nTuring\n";

let mut generator =
    NameGenerator::from_lines(first_names, last_names, Rng::from_entropy())
        .expect("word lists should be clean");

println!("{}", generator.generate());
```

`TemplateGenerator::from_lines` takes the same pattern-and-slots shape as
`TemplateGenerator::new`, just with each slot's word list as a block of
text instead of a `Vec<String>`. Both types also have
`with_strictness_from_lines` for `Strictness::Lenient` input. If you
already have parsed strings in hand and just want the line-splitting
logic, `parse_word_list` is exposed directly.

## Patterns beyond "first last"

`NameGenerator` only ever combines a first name and a last name. For
anything else - a title, a quoted nickname, syllables stacked into a
fantasy name - use `TemplateGenerator`, which takes a pattern string and
any number of named slots:

```rust
use namesmith::{Rng, TemplateGenerator};

let mut generator = TemplateGenerator::new(
    "{title} {first} \"{nickname}\" {last}",
    vec![
        ("title", vec!["Capt.".to_string(), "Dr.".to_string()]),
        ("first", vec!["Grace".to_string(), "Ada".to_string()]),
        ("nickname", vec!["Amazing".to_string(), "Bug-finder".to_string()]),
        ("last", vec!["Hopper".to_string(), "Lovelace".to_string()]),
    ],
    Rng::from_entropy(),
)
.expect("word lists should be clean");

println!("{}", generator.generate());
```

Every slot's word list goes through the same strict-by-default validation
as `NameGenerator`'s lists. A pattern that references a slot you didn't
provide, or a `{` with no matching `}`, is also rejected at build time
rather than producing a garbled name later.

## Weighting entries

By default every entry in a list is equally likely. If some names should
come up more often - a handful of common surnames next to a long tail of
rare ones - use the `_weighted` constructors and pair each entry with a
weight:

```rust
use namesmith::{NameGenerator, Rng};

let last_names = vec![
    ("Smith".to_string(), 20),
    ("Okafor".to_string(), 5),
    ("Yamamoto".to_string(), 1),
];

let mut generator = NameGenerator::new_weighted(
    vec![("Ada".to_string(), 1), ("Grace".to_string(), 1)],
    last_names,
    Rng::from_entropy(),
)
.expect("word lists should be clean");

println!("{}", generator.generate());
```

`"Smith"` comes up 20 times as often as `"Yamamoto"` here. A weight of zero
is rejected under `Strictness::Strict` (an entry that can never be picked
is almost always a typo), and dropped instead of erroring under
`Strictness::Lenient`. `TemplateGenerator` has the same pair of
`new_weighted` / `with_strictness_weighted` constructors for its slots.

## Markov chain synthesis

`NameGenerator` and `TemplateGenerator` both pick whole entries out of a
list you already wrote. `MarkovGenerator` does something different: it
trains on a set of example names and generates new ones that share their
letter patterns, without necessarily reproducing any of the inputs
verbatim. This is useful when a hand-written list would feel repetitive
with only a handful of entries - a dozen examples can already produce a
much larger variety of plausible-looking output.

```rust
use namesmith::{MarkovGenerator, Rng};

let examples = vec![
    "Aria".to_string(), "Ariana".to_string(), "Marina".to_string(),
    "Ada".to_string(), "Amara".to_string(), "Ariel".to_string(),
];

let mut generator = MarkovGenerator::new(examples, 2, Rng::from_entropy())
    .expect("training examples should be clean");

for _ in 0..5 {
    println!("{}", generator.generate());
}
```

The second argument is the chain's order: how many preceding characters it
looks at to pick the next one. Order 1 only knows which letter tends to
follow which and reads as noise; order 2 or 3 usually produces
name-shaped output; higher orders stay closer to the training data and
eventually reproduce it outright. `from_lines` loads examples from raw
text the same way the other generators do, and `with_max_length` caps how
long a generated name can get, in case the chain happens to cycle without
ever landing on its learned end-of-name transition.

Training examples go through a lighter check than `NameGenerator`'s lists:
an empty entry or a disallowed character is still rejected, but repeats
are fine (they're exactly how a pattern gets weighted more heavily) and
there's no `Strictness` knob, since trimming or de-duplicating the input
would just be throwing training data away.

## Design notes

- Zero dependencies. The crate ships its own seedable PRNG
  ([SplitMix64](https://prng.di.unimi.it/splitmix64.c)) instead of pulling
  in `rand`, since the whole point of the library is to be a small,
  auditable piece of a larger project.
- `Rng::from_seed` gives reproducible output for tests and snapshots.
  `Rng::from_entropy` seeds from wall-clock time for everyday use.
- This is a library, not a CLI. Wire it up wherever you need names:
  test fixtures, seed data, procedurally generated game content.

## Status

Early. `NameGenerator` covers "first + last" pairs, `TemplateGenerator`
covers arbitrary patterns, both support weighted entries, and
`MarkovGenerator` covers synthesizing new names from examples rather than
picking from a fixed list. See the roadmap in commit history for what's
planned.

## License

MIT, see [LICENSE](LICENSE).
