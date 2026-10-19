# Contributing to yorc-rs

Bug reports and pull requests are welcome. Keep pull requests small, and add
a test for every change in behaviour.

## Repository layout

| Path | What |
|---|---|
| `src/` | The `yorc` crate: `id`, `model`, `xml`, `json`, `hash`, `rows`, `dump`, `blocking` (one module per cargo feature; see the README) |
| `examples/` | One small program per module, and `walkthrough.rs`, the cells of [WALKTHROUGH.md](WALKTHROUGH.md) |
| `tests/` | Integration tests: every ORCID sample read, written and read back (XML and JSON), the schema walk, section rows, the client against a local mock server, the walkthrough |
| `tests/data/` | A real public record (CC0), snapshots for the diff, group-id records and a small Public Data File archive; see [NOTICE](NOTICE) |
| `reference/orcid-model/` | ORCID's v3.0 XSDs and samples, copied unmodified (ORCID's MIT-style licence; `PROVENANCE.md` names the commit) |
| `ARCHITECTURE.md` | Design rules and the state of each module |

Only `src/`, `examples/` and the top-level documents are packaged
(`include` in `Cargo.toml`); `tests/` and `reference/` stay in the
repository.

## Checks before a pull request

```sh
cargo fmt --check
cargo test --all-features          # every test and doctest; no network
cargo test                         # default features (id, model, xml)
cargo test --no-default-features
cargo clippy --all-features --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

CI runs the same on Linux, macOS and Windows, and the tests on the minimum
supported Rust version (README, "Minimum supported Rust version").

With `xmllint` (libxml2) installed, the round-trip tests also validate every
written document against the vendored XSDs; without it they say the check
was skipped. The README's Rust blocks are compiled as doctests, and
`tests/walkthrough.rs` compares WALKTHROUGH.md with the example's output, so
update both together.

## Rules the code keeps

- **Schema-derived.** Every data-bearing element and attribute of the v3.0
  record XSDs has a model field; `tests/schema_coverage.rs` walks the XSDs.
  A new field needs its XML and JSON read and write, and a sample that
  carries it.
- **Exact writes.** Written XML and JSON are deterministic, and the same
  bytes as yorc-zig's for the same model: a change to the output format is
  a change to both libraries.
- **No panics on data.** Malformed or unexpected input is an `Err` or an
  `Other(..)` value, never a panic.
- **Written from the specifications.** No code is copied from other ORCID
  software (see [COMPARISON.md](COMPARISON.md) and [NOTICE](NOTICE)).
- **Reads only.** The client does not write to ORCID, and keeps to one
  request every 45 s to ORCID's hosts.

## Benchmarks

[BENCHMARKS.md](BENCHMARKS.md) has the published figures, how they were
measured and their caveats.

## Releases

Versions follow Semantic Versioning (README, "Versioning"); every release
has an entry in [CHANGELOG.md](CHANGELOG.md) and a tag `v<version>`.

## Licence of contributions

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in yorc-rs by you shall be licensed as MIT, without
any additional terms or conditions.
