//! Keeps AppImage launcher state out of the processes Neptune starts.
//!
//! The AppImage runtime exports `ARGV0`, `APPIMAGE`, `APPDIR` and `OWD`, and
//! `AppRun` prepends the bundled window libraries to `LD_LIBRARY_PATH`. Shells
//! and helpers would inherit all of it: zsh uses an exported `ARGV0` as
//! `argv[0]` of every command, which breaks multi-call binaries such as rustup's
//! `cargo`, and host programs would load Neptune's bundled libraries. An
//! extracted AppDir (e.g. a `.desktop` whose `Exec=` points at `AppRun`)
//! exports `LD_LIBRARY_PATH` and sets `APPDIR` but never `APPIMAGE`, so the
//! cleanup below fires there as well.
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::LazyLock,
};

static APPIMAGE: LazyLock<Option<PathBuf>> =
    LazyLock::new(|| std::env::var_os("APPIMAGE").map(PathBuf::from));

/// The AppImage file this process was launched from, read before [`restore`]
/// takes it out of the environment.
pub fn appimage_path() -> Option<&'static Path> {
    APPIMAGE.as_deref()
}

/// Whether this process was launched from an AppImage.
pub fn appimage() -> bool {
    appimage_path().is_some()
}

/// Restore the environment the user launched Neptune from.
///
/// Fires on either of two launch shapes:
/// - a real AppImage (the runtime sets `APPIMAGE` and `APPDIR`, and `AppRun`
///   prepends the bundled libraries to `LD_LIBRARY_PATH`); or
/// - an extracted AppDir (a `.desktop` points `Exec=` at `AppRun`, which sets
///   `APPDIR` and exports `LD_LIBRARY_PATH` but never `APPIMAGE`).
///
/// In both cases `LD_LIBRARY_PATH` is rewritten to its host-only value and the
/// launcher bookkeeping variables are removed.
///
/// The dynamic loader reads `LD_LIBRARY_PATH` once at startup, so Neptune keeps
/// resolving its bundled libraries after the variable is restored.
///
/// # Safety
/// Must run before any other thread exists; it mutates the process environment.
pub unsafe fn restore() {
    let appdir = std::env::var_os("APPDIR");
    if !appimage() && appdir.is_none() {
        return;
    }
    let libraries = std::env::var_os("LD_LIBRARY_PATH")
        .zip(appdir.as_deref())
        .map(|(path, appdir)| host_library_path(&path, Path::new(appdir)));
    // SAFETY: the caller guarantees no other thread reads the environment.
    unsafe {
        match libraries {
            Some(Some(path)) => std::env::set_var("LD_LIBRARY_PATH", path),
            Some(None) => std::env::remove_var("LD_LIBRARY_PATH"),
            None => {}
        }
        for name in ["ARGV0", "APPIMAGE", "APPDIR", "OWD"] {
            std::env::remove_var(name);
        }
    }
}

/// `path` without the entries inside `appdir`; `None` when nothing remains.
fn host_library_path(path: &OsStr, appdir: &Path) -> Option<OsString> {
    let host: Vec<_> = std::env::split_paths(path)
        .filter(|entry| !entry.starts_with(appdir))
        .collect();
    if host.is_empty() {
        return None;
    }
    std::env::join_paths(host).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_libraries_leave_the_child_library_path() {
        let appdir = Path::new("/tmp/.mount_neptunAbCdEf");
        let host = |path: &str| host_library_path(OsStr::new(path), appdir);
        assert_eq!(host("/tmp/.mount_neptunAbCdEf/usr/lib"), None);
        assert_eq!(
            host("/tmp/.mount_neptunAbCdEf/usr/lib:/opt/cuda/lib64:/usr/local/lib"),
            Some("/opt/cuda/lib64:/usr/local/lib".into())
        );
        // A sibling mount of another AppImage is the user's own setting.
        assert_eq!(
            host("/tmp/.mount_neptunAbCdEfg/usr/lib"),
            Some("/tmp/.mount_neptunAbCdEfg/usr/lib".into())
        );
    }

    #[test]
    fn extracted_appdir_launch_cleans_bundled_library_path() {
        // `appimage()` reads APPIMAGE through a LazyLock that captures the
        // first observed value for the rest of the process. Make sure the
        // cache observes an empty APPIMAGE before we stage the extracted-AppDir
        // state, so the new code path (APPDIR-driven strip) is what runs.
        // SAFETY: rustc's default test runner executes each test on a single
        // thread, so no other code reads the process environment here.
        unsafe { std::env::remove_var("APPIMAGE") };
        let _ = appimage();

        let mount = PathBuf::from("/tmp/.mount_neptunAbCdEf");
        let stashed: [(&str, Option<OsString>); 5] = [
            ("APPDIR", std::env::var_os("APPDIR")),
            ("APPIMAGE", std::env::var_os("APPIMAGE")),
            ("ARGV0", std::env::var_os("ARGV0")),
            ("OWD", std::env::var_os("OWD")),
            ("LD_LIBRARY_PATH", std::env::var_os("LD_LIBRARY_PATH")),
        ];

        // SAFETY: see comment above.
        unsafe {
            std::env::set_var("APPDIR", &mount);
            std::env::set_var("ARGV0", "/tmp/.mount_neptunAbCdEf/AppRun");
            std::env::set_var("OWD", "/tmp");
            std::env::set_var("LD_LIBRARY_PATH", mount.join("usr/lib").as_os_str());
        }

        // SAFETY: see comment above.
        unsafe { restore() };

        for (key, _) in &stashed {
            assert!(
                std::env::var_os(key).is_none(),
                "{key} should have been removed by restore(), was still set",
            );
        }

        // Restore the original env so subsequent tests aren't polluted.
        // SAFETY: see comment above.
        unsafe {
            for (key, saved) in stashed {
                match saved {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}
