use glob::glob;
use std::fs;
use std::io::Error as IoError;
use std::path::{Path, PathBuf, StripPrefixError};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Couldn't resolve glob: '{glob}'")]
    GlobError {
        source: glob::PatternError,
        glob: String,
    },

    #[error("Expected file '{path:?}' to exist")]
    MissingFileError { path: String },

    #[error("Expected directory '{path:?}' to exist")]
    MissingDirectoryError { path: String },
}
pub(crate) fn get_relative_path_string(
    file_path: impl AsRef<Path>,
    base_path: impl AsRef<Path>,
) -> Result<String, StripPrefixError> {
    let rel_path = file_path
        .as_ref()
        .strip_prefix(base_path.as_ref())
        .map(|path| path_to_string(path))?;

    Ok(rel_path)
}
pub(crate) fn get_paths_from_glob(pattern: &String) -> Result<Vec<PathBuf>, Error> {
    let paths = glob(&pattern)
        .map_err(|e| Error::GlobError {
            source: e,
            glob: pattern.to_string(),
        })?
        .filter_map(|path| path.ok())
        .collect();

    Ok(paths)
}
pub(crate) fn expect_file(path: impl AsRef<Path>) -> Result<(), Error> {
    if !path.as_ref().exists() {
        return Err(Error::MissingFileError {
            path: path_to_string(path),
        });
    }
    Ok(())
}
pub(crate) fn expect_directory(path: impl AsRef<Path>) -> Result<(), Error> {
    if !path.as_ref().is_dir() {
        return Err(Error::MissingDirectoryError {
            path: path_to_string(path),
        });
    }
    Ok(())
}
pub(crate) fn ensure_directory(path: impl AsRef<Path>) -> Result<(), IoError> {
    fs::create_dir_all(&path)
}
pub(crate) fn read_file_contents(path: impl AsRef<Path>) -> Result<String, IoError> {
    fs::read_to_string(path)
}
pub(crate) fn write_file_contents(content: &String, path: impl AsRef<Path>) -> Result<(), IoError> {
    fs::write(path, content)
}

fn path_to_string(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().to_string()
}
