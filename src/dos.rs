use std::path::Path;

/// Check if filename match DOS constraint.
///
/// - File stem must have max 8 chars
/// - Extension must have max 3 chars
/// - Must contains only ASCII chars
pub fn is_valid_filename<P>(path: P) -> bool
where
    P: AsRef<Path>,
{
    let p = path.as_ref();
    let is_ascii = p.to_str().map(|f| f.is_ascii()).unwrap_or(false);
    let is_stem_valid = p.file_stem().map(|f| f.len() <= 8).unwrap_or(false);
    let is_ext_valid = p.extension().map(|f| f.len() <= 3).unwrap_or(true);

    is_ascii && is_stem_valid && is_ext_valid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_path() {
        // No extension
        assert!(is_valid_filename("TOTO"));
        assert!(is_valid_filename("toto"));

        // Not so long
        assert!(is_valid_filename("TOTO.MD"));
        assert!(is_valid_filename("toto.md"));

        // Max allowed size
        assert!(is_valid_filename("TOTOTOTO.TXT"));
        assert!(is_valid_filename("totototo.txt"));
    }

    #[test]
    fn test_invalid_path() {
        // Extension too long
        assert!(!is_valid_filename("TOTO.DOCX"));

        // Invalid ASCII chars
        assert!(!is_valid_filename("éà.TXT"));

        // Filename too long
        assert!(!is_valid_filename("TOTO_TOTO.DOC"));

        // Lot of sub-extension
        assert!(!is_valid_filename("TOTO.TAR.GZ.BZ2"));
    }
}
