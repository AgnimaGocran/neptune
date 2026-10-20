//! Keeps AppImage launcher state out of the processes Neptune starts.
//!
//! The AppImage runtime exports `ARGV0`, `APPIMAGE`, `APPDIR` and `OWD`, and
//! `AppRun` prepends the bundled window libraries to `LD_LIBRARY_PATH`. Shells
//! and helpers would inherit all of it: zsh uses an exported `ARGV0` as
//! `argv[0]` of every command, which breaks multi-call binaries such as rustup's
//! `cargo`, and host programs would load Neptune's bundled libraries. An
//! extracted AppDir (e.g. a `.desktop` whose `Exec=` points at `AppRun`)
//! exports `LD_LIBRARY_PATH` but never `APPIMAGE`, so the cleanup below fires
//! there as well; when a launcher exports no `APPDIR` at all, the bundle is
//! still recognized from the libraries sitting beside this executable.
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
/// Fires on any of three launch shapes:
/// - a real AppImage (the runtime sets `APPIMAGE` and `APPDIR`, and `AppRun`
///   prepends the bundled libraries to `LD_LIBRARY_PATH`);
/// - an extracted AppDir (a `.desktop` points `Exec=` at `AppRun`, which
///   exports `APPDIR` and `LD_LIBRARY_PATH` but never `APPIMAGE`); or
/// - a launcher that exports only `LD_LIBRARY_PATH`: the bundle is then
///   recognized from the bundled library directory beside this executable.
///
/// In every case `LD_LIBRARY_PATH` is rewritten to its host-only value and the
/// launcher bookkeeping variables are removed.
///
/// The dynamic loader reads `LD_LIBRARY_PATH` once at startup, so Neptune keeps
/// resolving its bundled libraries after the variable is restored.
///
/// # Safety
/// Must run before any other thread exists; it mutates the process environment.
pub unsafe fn restore() {
    let appdir = std::env::var_os("APPDIR")
        .map(PathBuf::from)
        .or_else(bundled_appdir);
    if !appimage() && appdir.is_none() {
        return;
    }
    let libraries = std::env::var_os("LD_LIBRARY_PATH")
        .zip(appdir.as_deref())
        .map(|(path, appdir)| host_library_path(&path, appdir));
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

/// The AppDir this executable belongs to, when no launcher marker was
/// exported: `<appdir>/usr/bin/neptune` with `AppRun` beside it names the
/// bundle, and `<appdir>/usr/lib` on `LD_LIBRARY_PATH` names the leak. Older
/// `AppRun` scripts kept `APPDIR` in the shell and exported only the library
/// path, and hand-written units can do the same; the shape of the launch is
/// what matters, not the marker.
fn bundled_appdir() -> Option<PathBuf> {
    let libraries = std::env::var_os("LD_LIBRARY_PATH")?;
    let exe = std::env::current_exe().ok()?;
    let appdir = bundled_appdir_at(&libraries, &exe)?;
    if !appdir.join("AppRun").is_file() {
        return None;
    }
    Some(appdir)
}

/// The `usr` directory of the bundle layout `exe` sits in, if it does.
fn bundle_usr(exe: &Path) -> Option<&Path> {
    let bin = exe.parent()?;
    if bin.file_name()? != OsStr::new("bin") {
        return None;
    }
    let usr = bin.parent()?;
    if usr.file_name()? != OsStr::new("usr") {
        return None;
    }
    Some(usr)
}

/// The shape check of [`bundled_appdir`] without the `AppRun` lookup.
fn bundled_appdir_at(libraries: &OsStr, exe: &Path) -> Option<PathBuf> {
    let usr = bundle_usr(exe)?;
    let libraries_dir = usr.join("lib");
    if !std::env::split_paths(libraries)
        .any(|entry| entry.starts_with(&libraries_dir))
    {
        return None;
    }
    usr.parent().map(Path::to_path_buf)
}

/// The AppDir beside this executable, whatever the environment says now.
///
/// `restore` removes the launcher markers, so later questions — such as
/// whether the browser subprocess may fall back to the bundle's libraries —
/// ask the filesystem instead of the environment.
pub fn bundle_beside_executable() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let appdir = bundle_usr(&exe)?.parent()?.to_path_buf();
    if !appdir.join("AppRun").is_file() {
        return None;
    }
    Some(appdir)
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

    #[test]
    fn a_bundle_without_exported_markers_is_found_from_the_executable() {
        let appdir = Path::new("/tmp/.mount_neptunAbCdEf");
        let exe = appdir.join("usr/bin/neptune");
        let detect = |libraries: &str| bundled_appdir_at(OsStr::new(libraries), &exe);
        assert_eq!(detect("/tmp/.mount_neptunAbCdEf/usr/lib"), Some(appdir.into()));
        // The browser subprocess libraries live deeper in the bundle.
        assert_eq!(
            detect("/tmp/.mount_neptunAbCdEf/usr/lib/neptune/browser"),
            Some(appdir.into())
        );
        // Host-only entries name no bundle.
        assert_eq!(detect("/opt/cuda/lib64:/usr/local/lib"), None);
        // The same shape as a system install is not decided here;
        // `bundled_appdir` also requires `AppRun` beside the directory.
        assert_eq!(
            bundled_appdir_at(OsStr::new("/usr/lib"), Path::new("/usr/bin/neptune")),
            Some(PathBuf::from("/"))
        );
        // A binary outside a usr/bin layout is no bundle.
        assert_eq!(
            bundled_appdir_at(
                OsStr::new("/home/u/.local/opt/neptune/usr/lib"),
                Path::new("/home/u/.local/opt/neptune/neptune"),
            ),
            None
        );
    }
}
