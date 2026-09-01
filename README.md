# `cfail` — simple in-line compile-fail tests

This crate lets you test that certain code patterns *don't* compile.

Disclaimer: very early in development.

## Installation

Add `cfail` in the `[dev-dependencies]` section:

```shell
$ cargo add --dev cfail
```

Allow use of `#[cfg(compile_fail)]` without warnings (optional):

```toml
// Cargo.toml
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(compile_fail)'] }
```

Call `cfail::run()` from an integration test:

```rust
// tests/cfail.rs
#[test]
fn cfail() {
    cfail::run()
}
```

Write a compile-fail test (can be placed in any unit or integration test in the
workspace):

```rust
#[test]
fn compiles_and_runs() {
    assert_eq!(1 + 1, 2);
}

#[test]
#[should_panic = "assertion failed"]
fn compiles_and_panics() {
    assert_eq!(1 + 1, 5);
}

#[test]
#[cfg(compile_fail)]
fn fails_to_compile() {
    // This statement will produce a "error[E0308]: mismatched types" diagnostic.
    // We can test for that by adding a cfail annotation with the error code `E0308`
    // on the line we expect the diagnostic on.

    let () = 0; //~ E0308
}
```

When the `cfail` test we defined above runs, `cfail` will build the crate with
`#[cfg(compile_fail)]` enabled, parse all compiler error messages, and validate
that they match the error annotations in your source files.

Error annotations are comments of the form `//~ E1234` containing a rustc
[error code].

[error code]: https://doc.rust-lang.org/error_codes/error-index.html
