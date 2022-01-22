use glob::glob;
use pathdiff::diff_paths;
use std::{
    fs,
    io::Error as IoError,
    path::{Path, PathBuf, StripPrefixError},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Couldn't resolve glob: '{glob}'")]
    GlobError {
        source: glob::PatternError,
        glob: String,
    },

    #[error("Couldn't find a diff from '{from}' to '{to}'")]
    PathDiffError { from: String, to: String },

    #[error("Expected file '{path:?}' to exist")]
    MissingFileError { path: String },

    #[error("Expected directory '{path:?}' to exist")]
    MissingDirectoryError { path: String },
}
pub(crate) fn get_stripped_base_path_string(
    file_path: impl AsRef<Path>,
    base_path: impl AsRef<Path>,
) -> Result<String, StripPrefixError> {
    let rel_path = file_path
        .as_ref()
        .strip_prefix(base_path.as_ref())
        .map(|path| path_to_string(path))?;

    Ok(rel_path)
}
pub(crate) fn get_relative_path_string(
    file_path: impl AsRef<Path>,
    base_path: impl AsRef<Path>,
) -> Result<String, Error> {
    let diff = match diff_paths(&file_path, &base_path) {
        Some(diff) => match path_to_string(diff) {
            diff if diff.is_empty() => ".".to_owned(),
            diff => diff,
        },
        None => {
            return Err(Error::PathDiffError {
                from: path_to_string(file_path),
                to: path_to_string(base_path),
            })
        }
    };

    Ok(path_to_string(diff))
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

#[cfg(test)]
mod tests {
    use super::get_relative_path_string;

    #[test]
    fn test_relative_path_nested_dir() {
        let dest_path = "site/posts";
        let base_path = "site";
        let diff_path = get_relative_path_string(&base_path, &dest_path).unwrap();
        assert_eq!(diff_path, "..");
    }

    #[test]
    fn test_relative_path_same_dir() {
        let dest_path = "site";
        let base_path = "site";
        let diff_path = get_relative_path_string(&base_path, &dest_path).unwrap();
        assert_eq!(diff_path, ".");
    }
}
