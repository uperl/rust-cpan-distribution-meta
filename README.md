# cpan-distribution-meta

Parse CPAN distribution meta files (`META.json` / `META.yml`) into a typed Rust
structure.

The [CPAN Meta Spec][spec] has several revisions: `META.yml` files use the `1.x`
series, `META.json` files use version `2`. This crate accepts **any** of those
versions in **either** JSON or YAML, and always returns a [`Meta`] value
normalised to the version 2 structure, so consumers only ever deal with one
shape. The spec version of the original document is preserved on
`Meta::spec_version`.

Legacy metadata is upgraded following the same rules as Perl's
[`CPAN::Meta::Converter`][conv]: flat `requires` / `build_requires` /
`configure_requires` / `recommends` / `conflicts` become the phase/relationship
`prereqs` map, a single `license` string becomes a list of normalised license
tokens, string `resources` become structured `repository` / `bugtracker`
values, the `private` field becomes `no_index`, and so on.

## Usage

```rust
use cpan_distribution_meta::parse;

let meta = parse(std::fs::read_to_string("META.json")?.as_str())?;

println!("{} {}", meta.name, meta.version);
for (module, range) in &meta.prereqs.runtime.requires {
    println!("  requires {module} {range}");
}
```

Entry points:

| function | input |
| --- | --- |
| `parse(&str)` | JSON or YAML, format auto-detected from content |
| `parse_json(&str)` / `parse_yaml(&str)` | a known format |
| `parse_slice(&[u8])` | UTF-8 bytes, BOM tolerated |
| `from_reader(impl Read)` | any reader |
| `from_path(impl AsRef<Path>)` | a file on disk |

`Meta` implements `serde::Serialize`, so a parsed document can be re-emitted as
canonical version 2 JSON with `serde_json`.

The `version_range` module parses prerequisite version strings
(`">= 1.2, != 1.5"`) into structured constraints.

## Notes

* `META.yml` conventionally quotes version numbers. An *unquoted* `version: 1.10`
  is parsed by the YAML layer as the float `1.1`, losing the trailing zero (as
  with most YAML parsers). Quote version-like scalars.
* Unrecognised license tokens are normalised to `unknown`.
* Custom `x_*` keys and any other unrecognised keys are preserved verbatim in
  `Meta::custom` (and `Resources::custom`).

[spec]: https://metacpan.org/pod/CPAN::Meta::Spec
[conv]: https://metacpan.org/pod/CPAN::Meta::Converter
[`Meta`]: https://docs.rs/cpan-distribution-meta/latest/cpan_distribution_meta/struct.Meta.html
