use core::error::Error;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use serde_json::Value;

use crate::annotation::AnnotationCollector;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

macro_rules! bail {
    ($($t:tt)*) => {
        return Err(format!($($t)*).into())
    };
}

mod annotation;

fn cargo() -> Command {
    Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

/// Runs `cfail` with the default configuration.
pub fn run() {
    Config::new().unwrap().run_tests().unwrap()
}

/// `cfail` configuration builder.
pub struct Config {
    ann: AnnotationCollector,
}

impl Config {
    /// Creates a new [`Config`] object.
    ///
    /// This will invoke `cargo metadata` to learn about the workspace layout, and populates
    /// the search path for annotated source files.
    pub fn new() -> Result<Config> {
        Ok(Config {
            ann: AnnotationCollector::new()?,
        })
    }

    /// Excludes a directory from annotation parsing.
    ///
    /// Normally, all directories that contain a Rust source file that forms the root of a module
    /// tree (eg. `lib.rs`) will be parsed recursively.
    /// This method can be used to exclude a directory from processing.
    pub fn exclude_dir<P: AsRef<Path>>(&mut self, dir: P) -> Result<&mut Self> {
        self.ann.exclude_dir(dir.as_ref())?;
        Ok(self)
    }

    /// Runs the `cfail` tests.
    ///
    /// This will invoke `cargo test` with `#[cfg(compile_fail)]` enabled and parse all compiler
    /// error diagnostics.
    ///
    /// Tests will fail if a `//~ E1234` annotation specifies an error that wasn't produced at that
    /// location, or if the compiler produces an error that has no matching annotation.
    ///
    /// Tests will also fail if no `//~ E1234` annotations were found, or if `cargo test` actually
    /// succeeds instead of exiting with an error status.
    pub fn run_tests(&self) -> Result<()> {
        let mut annotations = self.ann.collect_annotations()?;

        // `--no-fail-fast` is important here. If we don't pass that, the set of diagnostics we
        // are handed will be non-deterministic and often incomplete.
        let cargo = cargo()
            .args([
                "test",
                "--workspace",
                "--no-fail-fast",
                "--message-format=json",
            ])
            .env("RUSTFLAGS", "--cfg compile_fail")
            .output()?;
        if cargo.status.success() {
            bail!("`cargo test` succeeded, but should have failed");
        }

        let output = String::from_utf8(cargo.stdout)?;
        for line in output.lines() {
            // https://doc.rust-lang.org/cargo/reference/external-tools.html#json-messages
            // Recommends skipping any lines that don't begin with {
            if !line.starts_with('{') {
                continue;
            }

            let value: Value = serde_json::from_str(line)?;
            if value.get("reason").and_then(|v| v.as_str()) == Some("compiler-message")
                && let Some(message) = value.get("message")
            {
                let msg = Message::deserialize(message)?;
                if msg.level != "error" {
                    continue;
                }

                let Some(code) = msg.code else {
                    bail!("error message without error code: {}", msg.message);
                };
                let primary = msg
                    .spans
                    .iter()
                    .filter(|s| s.is_primary)
                    .collect::<Vec<_>>();
                let primary = match &*primary {
                    [span] => *span,
                    [] => bail!("error message without primary span: {}", msg.message),
                    [..] => bail!("error message with multiple primary spans: {}", msg.message),
                };

                let path = primary.file_name.canonicalize()?;

                let mut found = false;
                for diag in annotations.query(&path, primary.line_start) {
                    if diag.code == code.code {
                        found = true;
                        diag.userdata = true;
                    }
                }

                if !found {
                    bail!(
                        "encountered compiler error without matching annotation at {}:{}: {} {}",
                        primary.file_name.display(),
                        primary.line_start,
                        code.code,
                        msg.message,
                    );
                }
            }
        }

        // Flag all annotations that weren't matched to a diagnostic
        for (loc, diag) in annotations.iter_mut() {
            if !diag.userdata {
                bail!(
                    "compiler failed to produce annotated error {} at {}",
                    diag.code,
                    loc,
                );
            }
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct Message<'a> {
    level: &'a str,
    message: &'a str,
    spans: Vec<Span>,
    #[serde(borrow)]
    code: Option<Code<'a>>,
}

#[derive(Deserialize)]
struct Code<'a> {
    code: &'a str,
}

#[derive(Deserialize)]
struct Span {
    is_primary: bool,
    file_name: PathBuf,
    line_start: usize,
}
