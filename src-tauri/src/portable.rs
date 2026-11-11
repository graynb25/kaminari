// Portable-build detection. The zip produced by scripts/package-portable.mjs
// contains PORTABLE.txt next to Kaminari.exe and has no installer, so it cannot
// rely on installer-registered capabilities such as the kaminari:// handler.

use std::path::PathBuf;

const PORTABLE_ENV: &str = "KAMINARI_PORTABLE";
const PORTABLE_MARKER: &str = "PORTABLE.txt";

pub fn is_portable() -> bool {
    if let Some(value) = std::env::var_os(PORTABLE_ENV) {
        let value = value.to_string_lossy();
        let value = value.trim();
        if !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false") {
            return true;
        }
    }
    marker_path().is_some_and(|path| path.is_file())
}

fn marker_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(PORTABLE_MARKER))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_lives_next_to_the_executable() {
        let path = marker_path().expect("test executable has a parent");
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(PORTABLE_MARKER)
        );
    }
}
