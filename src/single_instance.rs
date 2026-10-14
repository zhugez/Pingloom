//! Per-user OS lock. Never unlink the file: replacing its inode would bypass the lock.
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io,
    path::Path,
};

pub(crate) fn acquire(directory: &Path) -> io::Result<File> {
    fs::create_dir_all(directory)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(directory.join(".instance.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Pingloom is already running for this user",
        )),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn child_process_probe() {
        let Some(directory) = std::env::var_os("PINGLOOM_TEST_LOCK_DIR") else {
            return;
        };
        let result = acquire(Path::new(&directory));
        match std::env::var("PINGLOOM_TEST_LOCK_ACTION").unwrap().as_str() {
            "blocked" => assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists),
            "crash" => {
                let _lock = result.unwrap();
                std::process::exit(0);
            }
            _ => panic!("unknown test action"),
        }
    }

    #[test]
    fn only_one_process_and_exit_releases_lock_without_stale_file() {
        let directory = tempfile::tempdir().unwrap();
        let probe = |action| {
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "single_instance::tests::child_process_probe"])
                .env("PINGLOOM_TEST_LOCK_DIR", directory.path())
                .env("PINGLOOM_TEST_LOCK_ACTION", action)
                .status()
                .unwrap()
                .success()
        };
        let lock = acquire(directory.path()).unwrap();
        assert!(probe("blocked"));
        drop(lock);
        assert!(probe("crash")); // exits without running the File destructor
        let _lock = acquire(directory.path()).unwrap();
        assert!(probe("blocked"));
    }
}
