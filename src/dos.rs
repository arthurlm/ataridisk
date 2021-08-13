use std::path::Path;

use crate::error::{self, SerialDiskError};

macro_rules! split_os_str {
    ($x:expr, $size:expr) => {{
        let s = $x.to_str().unwrap();
        let l = if s.len() > $size { $size } else { s.len() };
        s[..l].to_string()
    }};
}

/// Convert path into valid DOS components and return
/// filename (8 bytes) and extension (3 bytes).
///
/// It fails if filename contains not ASCII chars.
pub fn as_valid_file_components<P>(path: P) -> error::Result<(String, String)>
where
    P: AsRef<Path>,
{
    let p = path.as_ref();
    let file_stem = p.file_stem().ok_or(SerialDiskError::InvalidFilename)?;
    let extension = p.extension().unwrap_or_default();

    if !file_stem.is_ascii() || !extension.is_ascii() {
        return Err(SerialDiskError::InvalidChars);
    }

    Ok((split_os_str!(file_stem, 8), split_os_str!(extension, 3)))
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! file_components {
        ($name:expr) => {
            file_components!($name, "")
        };
        ($name:expr, $ext:expr) => {
            Ok(($name.to_string(), $ext.to_string()))
        };
    }

    #[test]
    fn test_valid_path() {
        // No extension lower / upper cases
        assert_eq!(as_valid_file_components("TOTO"), file_components!("TOTO"));
        assert_eq!(as_valid_file_components("toto"), file_components!("toto"));
        assert_eq!(
            as_valid_file_components("TOTO.MD"),
            file_components!("TOTO", "MD")
        );
        assert_eq!(
            as_valid_file_components("toto.md"),
            file_components!("toto", "md")
        );

        // Max allowed size
        assert_eq!(
            as_valid_file_components("foo_bar_"),
            file_components!("foo_bar_")
        );
        assert_eq!(
            as_valid_file_components("foo_bar_.txt"),
            file_components!("foo_bar_", "txt")
        );

        // Above max size
        assert_eq!(
            as_valid_file_components("foo_bar_baz.jpeg"),
            file_components!("foo_bar_", "jpe")
        );
    }

    #[test]
    fn test_invalid_path() {
        // No filename
        assert_eq!(
            as_valid_file_components("."),
            Err(SerialDiskError::InvalidFilename)
        );

        // Invalid chars
        assert_eq!(
            as_valid_file_components("héhé.txt"),
            Err(SerialDiskError::InvalidChars)
        );
        assert_eq!(
            as_valid_file_components("foo.héhé"),
            Err(SerialDiskError::InvalidChars)
        );
    }
}
