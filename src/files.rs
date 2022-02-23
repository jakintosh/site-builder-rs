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

    #[error("Couldn't strip path: '{path}'")]
    PathStripError {
        source: StripPrefixError,
        path: PathBuf,
    },

    #[error("Couldn't find a diff from '{from}' to '{to}'")]
    PathDiffError { from: String, to: String },

    #[error("Couldn't get extension from '{path}'")]
    PathMissingExtensionError { path: String },

    #[error("Couldn't read '{path}' because it's not valid unicode")]
    PathNotUnicodeError { path: String },

    #[error("Couldn't read file at '{path}'")]
    FileReadError { source: IoError, path: String },

    #[error("Couldn't write file at '{path}'")]
    FileWriteError { source: IoError, path: String },

    #[error("Coudln't create directory at '{path:?}'")]
    CreateDirectoryError {
        source: std::io::Error,
        path: String,
    },

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
pub(crate) fn get_extension(path: impl AsRef<Path>) -> Result<String, Error> {
    let extension = match path.as_ref().extension() {
        Some(ext) => ext,
        None => {
            return Err(Error::PathMissingExtensionError {
                path: path_to_string(path),
            })
        }
    };

    let extension = match extension.to_str() {
        Some(ext) => ext,
        None => {
            return Err(Error::PathNotUnicodeError {
                path: path_to_string(path),
            })
        }
    };

    Ok(extension.to_owned())
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
pub(crate) fn ensure_directory(path: impl AsRef<Path>) -> Result<(), Error> {
    fs::create_dir_all(&path).map_err(|e| Error::CreateDirectoryError {
        source: e,
        path: path_to_string(path),
    })?;

    Ok(())
}

pub(crate) fn read_file_contents(path: impl AsRef<Path>) -> Result<String, Error> {
    let contents = fs::read_to_string(&path).map_err(|e| Error::FileReadError {
        source: e,
        path: path_to_string(path),
    })?;
    Ok(contents)
}
pub(crate) fn write_file_contents(content: &String, path: impl AsRef<Path>) -> Result<(), Error> {
    fs::write(&path, content).map_err(|e| Error::FileWriteError {
        source: e,
        path: path_to_string(path),
    })?;
    Ok(())
}

pub(crate) fn load_component_files(
    components_glob: &String,
    source_dir_path: &String,
) -> Result<Vec<(String, String)>, Error> {
    let mut components: Vec<(String, String)> = Vec::new();
    let component_paths = get_paths_from_glob(&components_glob)?;
    for path in component_paths {
        let component_name =
            get_stripped_base_path_string(&path, &source_dir_path).map_err(|e| {
                Error::PathStripError {
                    source: e,
                    path: path.clone(),
                }
            })?;
        let component = read_file_contents(path)?;
        components.push((component_name, component));
    }

    Ok(components)
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
