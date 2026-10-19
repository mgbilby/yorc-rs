# yorc-rs benchmarks and parity

Timings of yorc-rs next to the other implementations of the ORCID v3.0
message schema in the yorc family, [yorc-zig](https://github.com/mgbilby/yorc-zig)
and [yorc-duckdb](https://github.com/mgbilby/yorc-duckdb), with ORCID's own
Java library ([orcid-model](https://github.com/ORCID/orcid-model) 4.0.2, JAXB
and Jackson configured as ORCID's API configures them) as the baseline. Then
how the outputs of the four compare.

## Results

### Java, Rust and Zig, Rust and Zig on the same allocator (headline)

_2026-10-19, on the maintainer's machine: median of 5 runs, the order of the
programs rotating between runs; single-threaded. Rust: `--release`, fat LTO,
one codegen unit, `-C target-cpu=native`. Zig 0.18.0-dev.131+41f885830:
ReleaseFast for the native CPU. Rust and Zig both on the system allocator
(glibc malloc; the Zig program linked with libc), so the Rust/Zig ratio
compares the libraries' code. Java: ORCID's orcid-model 4.0.2 on JDK 21,
steady state after JIT warm-up._

This run predates the published runs. The latest published run, with its
raw lines and notes, is in [yorc-bench's RESULTS.md](https://github.com/mgbilby/yorc-bench/blob/main/RESULTS.md).

ns/op (lower is faster); A/B is A's time over B's, so below 1 A is faster.

| workload | Java | Rust | Zig | Rust/Java | Zig/Java | Zig/Rust |
|---|---:|---:|---:|---:|---:|---:|
| id_normalize | - | 771,358 | 413,013 | - | - | 0.54 |
| xml_read_record | 13,479,587 | 5,786,816 | 3,390,065 | 0.43 | 0.25 | 0.59 |
| xml_write_record | 5,409,463 | 589,521 | 581,692 | 0.11 | 0.11 | 0.99 |
| json_write_record | 4,295,744 | 4,902,113 | 3,609,261 | 1.14 | 0.84 | 0.74 |
| json_read_record | 4,036,027 | 5,526,203 | 1,769,885 | 1.37 | 0.44 | 0.32 |
| rows_record | - | 1,140,782 | 141,981 | - | - | 0.12 |
| sha256 | 3,113,553 | 4,110,109 | 3,979,678 | 1.32 | 1.28 | 0.97 |
| dump_stream | 15,621,551 | 6,778,430 | 3,917,354 | 0.43 | 0.25 | 0.58 |
| group_ids_read | 552 | 785 | 796 | 1.42 | 1.44 | 1.01 |
| timeline_record | - | 417,529 | 147,434 | - | - | 0.35 |
| changes_record | - | 2,570,608 | 527,083 | - | - | 0.21 |
| dump_sample | - | 704,356 | 794,058 | - | - | 1.13 |
| **average** (geometric mean) | | | | **0.67** | **0.46** | **0.52** |

`-`: ORCID's Java library has no counterpart (no iD utility, rows,
timeline, diff or dump reader). The Java columns average over the 7
workloads Java runs, Zig/Rust over all 12. yorc-zig's BENCHMARKS.md also
gives Zig on its own allocator.

What the run says about yorc-rs:

- It writes ORCID XML in 0.11 of Java's time (level with yorc-zig, 0.99),
  reads XML in 0.43 of Java's time and streams the dump fixture in 0.43.
- It trails Java at JSON (read 1.37, write 1.14), SHA-256 (1.32: the JDK's
  intrinsic against `sha2`, which picks SHA-NI at run time) and the very
  small group-id read (1.42, under a microsecond on every side). Its JSON
  reader is the slowest of the three libraries and the clearest target.
- On the same allocator yorc-zig takes about half its time on average
  (Zig/Rust 0.52) and is faster on 9 of the 12 workloads; yorc-rs leads only
  on the reservoir sample (1.13) and is level on XML writing, SHA-256 and
  the group-id read.

### All four implementations (2026-10-18, earlier run)

_2026-10-18, on the maintainer's machine, the same builds except that Zig ran
on its own `smp_allocator` (this run predates the same-allocator headline).
DuckDB: the v1.5.6 CLI with yorc-duckdb linked in, `SET threads = 1`.
Raw lines and notes: not published (this run predates
[yorc-bench](https://github.com/mgbilby/yorc-bench))._

| workload | Java | Rust | Zig (own allocator) | DuckDB (provisional) | Rust/Java | Zig/Rust |
|---|---:|---:|---:|---:|---:|---:|
| id_normalize | - | 776,062 | 447,670 | 1,845,000 | - | 0.58 |
| xml_read_record | 13,462,202 | 5,830,386 | 4,346,944 | 3,966,667 | 0.43 | 0.75 |
| xml_write_record | 5,350,607 | 596,104 | 1,023,642 | - | 0.11 | 1.72 |
| json_write_record | 4,307,037 | 4,877,996 | 4,071,753 | - | 1.13 | 0.83 |
| json_read_record | 4,061,310 | 5,607,829 | 2,226,314 | 40,966,667 | 1.38 | 0.40 |
| rows_record | - | 1,163,736 | 460,412 | 48,000,000 | - | 0.40 |
| sha256 | 3,108,650 | 4,092,053 | 3,980,834 | 226,667 † | 1.32 | 0.97 |
| dump_stream | 15,568,395 | 6,765,126 | 5,913,709 | 600,000 † | 0.43 | 0.87 |
| group_ids_read | 556 | 792 | 787 | 1,000 | 1.42 | 0.99 |
| timeline_record | - | 427,414 | 353,849 | 11,000,000 | - | 0.83 |
| changes_record | - | 2,626,577 | 1,223,644 | 6,833,333 | - | 0.47 |
| dump_sample | - | 706,061 | 854,750 | 150,000 † | - | 1.21 |
| **average** (geometric mean) | | | | | **0.67** | **0.76** |

yorc-duckdb has no record writer, so the two record-writing workloads do not
apply to it. **The DuckDB column is provisional.** Its `sha256`,
`dump_stream` and `dump_sample` figures (†) are implausibly fast, probably
because the query optimiser prunes or caches part of the work in those
statements; they are being re-measured and are left out of every claim on
this page. Its other figures time one SQL statement each, planning and
result handling included, and are the closest SQL counterpart of each
workload rather than the same operation.

## Workloads

Each operation runs on in-memory input unless it says otherwise; the inputs
ship in this repository.

| workload | one operation |
|---|---|
| `id_normalize` | normalise 10,000 generated iDs (canonical, URL, hyphenless, spaced and lowercase-`x` forms; one in eight invalid) |
| `xml_read_record` | parse the 1.3 MB real record `tests/data/0000-0003-0100-6634.xml` (382 works) into the model |
| `xml_write_record` | write that model back as ORCID XML |
| `json_write_record` | write it as ORCID JSON |
| `json_read_record` | parse that JSON back (kind `record`) |
| `rows_record` | flatten the parsed record into the section tables (`rows`) |
| `sha256` | SHA-256 hex of the record (`hash`) |
| `dump_stream` | open `tests/data/dump/orcid_test_summaries.tar.gz` from disk, stream every `.xml` member and parse it (warm page cache) |
| `group_ids_read` | read ORCID's JSON write sample `peer-review-group-id.json` |
| `timeline_record` | the record's dated events (`rows::Extra`, timeline) |
| `changes_record` | diff the record against a copy with one work title edited (`rows::changes`) |
| `dump_sample` | the seeded reservoir sample, n = 5, seed 42, over the dump fixture; kept members are not read |

The dump fixture has 8 entries, so `dump_stream` mostly measures archive
setup. On a 10,000-record corpus (450 MB of XML), an earlier run streamed
and parsed at 189 MB/s (yorc-zig: 255 MB/s).

## Caveats

- Absolute numbers depend on the machine; compare ratios within one run.
- In the headline both libraries use glibc malloc; the allocation patterns
  still differ by design (a tree of `String`s and `Vec`s here, one arena per
  document in yorc-zig).
- Zig ReleaseFast removes every safety check; a Rust release build keeps
  slice bounds checks.
- yorc-rs reads XML with quick-xml and JSON with serde_json; yorc-zig has
  its own XML scanner and uses `std.json`.
- Every result passes through `std::hint::black_box`; there is no benchmark
  harness on either side, so both time the same thing with the monotonic
  clock.

## Parity

In the 2026-10-18 four-way run, a cross-check fed the same inputs (the real record, ORCID's
53 read and 21 write samples, ORCID's JSON samples, 10,000 iDs and the dump
fixture) to every implementation and compared the outputs class by class:
written XML and JSON byte for byte, section rows, JSON reads, dump members,
SHA-256, group ids, timelines and changes.

- yorc-rs, yorc-zig and yorc-duckdb were identical in every compared class
  (yorc-duckdb's writers in their crosswalk mode, which emits yorc-rs's
  layout).
- ORCID's Java library, as the reference reader, read 74 of the 75 JSON
  documents the others wrote. The remaining entry is the library's own XSD
  check: ORCID's read sample `work-3.0.xml` has an empty `journal-title`,
  which the XSD's non-blank pattern rejects and yorc reads as absent.

The 2026-10-19 run repeated the check for Rust and Zig: identical (597
files), and ORCID's library again read 74 of the 75 JSON documents.

Byte-identical output with yorc-zig is a contract of both libraries: a
difference is a bug in one of them.

## Reproducing

The harness that produced these figures is public:
[yorc-bench](https://github.com/mgbilby/yorc-bench). It holds the three
small driver programs (Rust, Zig, Java) and the SQL leg for the DuckDB CLI
that run the workloads above on the same inputs, print one JSON line per
workload and summarise them, the cross-check that diffs their output trees,
the committed results of the published runs, and yorc-rs, yorc-zig and
yorc-duckdb as submodules at the commits measured:

```sh
git clone --recursive https://github.com/mgbilby/yorc-bench
cd yorc-bench
./run.sh                    # Rust and Zig: build, cross-check, benchmark
./run.sh --duckdb --java    # all four (after `make release` in yorc-duckdb; JDK 21, Maven)
```

Its README lists the prerequisites. Its [RESULTS.md](https://github.com/mgbilby/yorc-bench/blob/main/RESULTS.md) has the latest
published run (all four implementations, with the machine, toolchains and
commits) and every earlier one, each backed by its raw lines in `results/`.

Every input is in this repository (`tests/data/`, `reference/orcid-model/`).
To time one operation yourself, call the API on those files in a release
build with `-C target-cpu=native`, keep the result alive with
`std::hint::black_box`, and take the median of several runs.
