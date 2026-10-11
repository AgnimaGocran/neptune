//! Single-instance handoff. A second launch with a folder to open forwards it
//! over a per-data-root unix socket to the running instance, which adds a
//! workspace instead of opening another window.
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

/// FNV-1a over the data-root bytes: stable across runs, unlike `DefaultHasher`.
fn data_root_key(data_root: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data_root.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub fn socket_path(data_root: &Path) -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join(format!("neptune-{}.sock", data_root_key(data_root)))
}

/// Hand `folder` to the running instance. True when it accepted the connection
/// and read the path, so this launch should exit without opening a window.
pub fn forward(folder: &Path, data_root: &Path) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket_path(data_root)) else {
        return false;
    };
    let mut line = folder.as_os_str().as_encoded_bytes().to_vec();
    line.push(b'\n');
    stream.write_all(&line).is_ok()
}

/// Bind the instance socket and accept folders from later launches. `wake`
/// runs after each accepted folder so the UI thread polls the receiver.
/// `None` when the socket is taken (a same-storage instance already listens).
pub fn listen(
    data_root: &Path,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> Option<Receiver<PathBuf>> {
    let path = socket_path(data_root);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(_) => {
            // Taken by a live instance: keep out of its way.
            if UnixStream::connect(&path).is_ok() {
                return None;
            }
            // A socket left by a crashed run cannot accept; replace it.
            let _ = std::fs::remove_file(&path);
            UnixListener::bind(&path).ok()?
        }
    };
    let (sender, receiver) = std::sync::mpsc::channel();
    let serving = std::thread::Builder::new()
        .name("neptune-instance".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let Some(line) = BufReader::new(stream).lines().next() else {
                    continue;
                };
                let Ok(line) = line else { continue };
                let folder = PathBuf::from(
                    <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(
                        line.into_bytes(),
                    ),
                );
                if sender.send(folder).is_err() {
                    return;
                }
                wake();
            }
        })
        .ok();
    if serving.is_some() {
        Some(receiver)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarding_reaches_a_listener_of_the_same_storage() {
        let dir = tempfile::tempdir().unwrap();
        let data_root = dir.path().join("storage");
        let wake = Arc::new(|| {});
        let receiver = listen(&data_root, wake).expect("first listener binds");
        assert!(listen(&data_root, Arc::new(|| {})).is_none());

        let folder = dir.path().join("project folder");
        assert!(forward(&folder, &data_root));

        assert_eq!(receiver.recv().unwrap(), folder);
        // A different storage root is a different instance.
        assert!(!forward(&folder, &dir.path().join("other")));
        assert_eq!(socket_path(&data_root), socket_path(&data_root));
        assert_ne!(socket_path(&data_root), socket_path(&dir.path().join("o")));
    }
}
