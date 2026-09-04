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

Early. `NameGenerator` covers "first + last" pairs and `TemplateGenerator`
covers arbitrary patterns; entries still can't be weighted, so every list
entry is equally likely. See the roadmap in commit history for what's
planned.

## License

MIT, see [LICENSE](LICENSE).
