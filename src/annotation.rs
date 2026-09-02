use std::{
    collections::{BTreeSet, HashMap},
    ffi::OsStr,
    fmt,
    fs::{self, File},
    io::{BufReader, prelude::BufRead},
    path::{Path, PathBuf},
    process::Stdio,
};

use serde::Deserialize;

use crate::cargo;

#[derive(Debug)]
pub(crate) struct AnnotationCollector {
    search_dirs: BTreeSet<PathBuf>,
    excluded_prefixes: Vec<PathBuf>,
}

impl AnnotationCollector {
    /// Creates an annotation collector, configured using `cargo metadata` output.
    ///
    /// This will search every directory containing a package target, which will usually find all
    /// source files that contribute code to that target (ie. included modules).
    pub(crate) fn new() -> crate::Result<Self> {
        let meta = Metadata::get()?;

        let mut dirs = BTreeSet::new();
        for pkg in &meta.packages {
            for tgt in &pkg.targets {
                let path = Path::new(&tgt.src_path);
                let dir = path.parent().unwrap_or(Path::new("."));
                dirs.insert(dir.to_path_buf());
            }
        }

        Ok(Self {
            search_dirs: dirs,
            excluded_prefixes: vec![PathBuf::from(meta.target_directory)],
        })
    }

    pub(crate) fn exclude_dir(&mut self, path: &Path) -> crate::Result<&mut Self> {
        self.excluded_prefixes.push(
            path.canonicalize()
                .map_err(|e| err!("failed to canonicalize '{}': {e}", path.display()))?,
        );
        Ok(self)
    }

    pub(crate) fn collect_annotations<U>(&self) -> crate::Result<AnnotationMap<U>>
    where
        U: Default,
    {
        eprintln!("collecting cfail annotations: {self:?}");

        let vec = self.collect_annotations_vec()?;
        eprintln!("found {} annotations: {vec:?}", vec.len());

        if vec.is_empty() {
            bail!(
                "no cfail annotations found in any of the source files (use `//~ E0123` to add one)"
            );
        }

        let mut map = HashMap::new();
        for (loc, diag) in vec {
            let diag = Diagnostic {
                code: diag.code,
                userdata: U::default(),
            };
            map.entry(loc).or_insert(Vec::new()).push(diag);
        }

        Ok(AnnotationMap { map })
    }

    fn collect_annotations_vec(&self) -> crate::Result<Vec<(Location, Diagnostic)>> {
        let mut ann = Vec::new();
        for dir in &self.search_dirs {
            self.collect_annotations_in(dir, &mut ann)?;
        }
        Ok(ann)
    }

    fn collect_annotations_in(
        &self,
        dir: &Path,
        out: &mut Vec<(Location, Diagnostic)>,
    ) -> crate::Result<()> {
        for prefix in &self.excluded_prefixes {
            if dir.starts_with(prefix) {
                return Ok(());
            }
        }

        for res in fs::read_dir(dir)? {
            let ent = res?;
            let ty = ent.file_type()?;
            let path = ent.path();
            if ty.is_dir() {
                self.collect_annotations_in(&path, out)?;
            } else if path.extension() == Some(OsStr::new("rs")) {
                for (i, res) in BufReader::new(File::open(&path)?).lines().enumerate() {
                    let line = res?;
                    let lineno = i + 1;
                    if let Some((_, ann)) = line.rsplit_once("//~") {
                        let loc = Location {
                            file: path.clone(),
                            line: lineno,
                        };
                        let diag = Diagnostic::parse(&loc, ann)?;
                        out.push((loc, diag));
                    }
                }
            }
        }

        Ok(())
    }
}

pub(crate) struct Diagnostic<U = ()> {
    pub(crate) code: String,
    pub(crate) userdata: U,
}

impl Diagnostic {
    fn parse(loc: &Location, s: &str) -> crate::Result<Self> {
        let code_string = s.trim();
        match code_string
            .strip_prefix('E')
            .and_then(|digits| digits.parse::<u32>().ok())
        {
            Some(_) => Ok(Self {
                code: code_string.to_string(),
                userdata: (),
            }),
            None => bail!("{loc}: malformed cfail annotation; syntax: `//~ E0123`"),
        }
    }
}

impl fmt::Debug for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Diagnostic")
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}

pub(crate) struct AnnotationMap<U> {
    map: HashMap<Location, Vec<Diagnostic<U>>>,
}

impl<U> AnnotationMap<U> {
    pub(crate) fn query(
        &mut self,
        file: &Path,
        line: usize,
    ) -> impl Iterator<Item = &mut Diagnostic<U>> {
        let loc = Location {
            file: file.to_path_buf(),
            line,
        };
        self.map.get_mut(&loc).into_iter().flatten()
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (&Location, &mut Diagnostic<U>)> {
        self.map
            .iter_mut()
            .flat_map(|(loc, diag)| diag.iter_mut().map(move |diag| (loc, diag)))
    }
}

#[derive(PartialEq, Eq, Hash)]
pub(crate) struct Location {
    file: PathBuf,
    line: usize,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file.display(), self.line)
    }
}
impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    target_directory: String,
}

impl Metadata {
    fn get() -> crate::Result<Self> {
        let output = cargo()
            .args(["metadata", "--format-version=1", "--no-deps"])
            .stderr(Stdio::inherit()) // forward error output
            .output()
            .map_err(|e| err!("failed to run `cargo metadata`: {e}"))?;
        if !output.status.success() {
            bail!("`cargo metadata` exited with error: {}", output.status);
        }

        serde_json::from_slice(&output.stdout).map_err(Into::into)
    }
}

#[derive(Deserialize)]
struct Package {
    targets: Vec<PackageTarget>,
}

#[derive(Deserialize)]
struct PackageTarget {
    src_path: String,
}
