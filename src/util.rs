use std::path::{Path, PathBuf};

pub(crate) trait PathExt {
    fn canon(&self) -> crate::Result<PathBuf>;
}
impl PathExt for Path {
    fn canon(&self) -> crate::Result<PathBuf> {
        self.canonicalize()
            .map_err(|e| err!("failed to canonicalize '{}': {e}", self.display()))
    }
}
