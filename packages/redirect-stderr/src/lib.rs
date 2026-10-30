// Process-wide stderr redirection into a file, restored when the guard drops.
// It swaps the OS-level stderr (fd 2 / STD_ERROR_HANDLE), so output from C
// libraries linked into the process is captured as well as `eprintln!`.

use std::fs::File;
use std::io;

#[cfg(target_os = "windows")]
static mut WINAPI_STDERR_HANDLE: windows_sys::Win32::Foundation::HANDLE =
  std::ptr::null_mut();
/// The handle currently installed as stderr, owned by us. Windows'
/// `SetStdHandle` does not duplicate the handle it is given, so it has to be
/// kept open for as long as it is installed and closed on restore.
#[cfg(target_os = "windows")]
static mut WINAPI_INSTALLED_HANDLE: windows_sys::Win32::Foundation::HANDLE =
  std::ptr::null_mut();
#[cfg(not(target_os = "windows"))]
static mut UNIX_STDERR_HANDLE: i32 = -1;

/// Sends stderr into `file` until the returned guard is dropped, which puts
/// the original stderr back.
///
/// Redirections don't nest: only one may be active at a time.
#[must_use = "stderr is restored as soon as the guard is dropped"]
pub fn redirect_stderr_to_file(file: &File) -> io::Result<StderrRedirect> {
  redirect_stderr_to(file)?;
  Ok(StderrRedirect { _private: () })
}

/// Restores the original stderr on drop. See `redirect_stderr_to_file`.
pub struct StderrRedirect {
  _private: (),
}

impl Drop for StderrRedirect {
  fn drop(&mut self) {
    let _ = restore_stderr();
  }
}

fn redirect_stderr_to(target: &File) -> io::Result<()> {
  #[cfg(target_os = "windows")]
  {
    use std::os::windows::io::{AsRawHandle, IntoRawHandle};
    use windows_sys::Win32::Foundation::{
      CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
    };
    use windows_sys::Win32::System::Console::{STD_ERROR_HANDLE, SetStdHandle};

    // A handle of our own, so it outlives the caller's `File`.
    let handle = target.try_clone()?.into_raw_handle() as HANDLE;
    unsafe {
      // Ensure the handle is not inherited by child processes.
      SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);

      // Save the real stderr only the first time, so a repeated redirect
      // doesn't overwrite it with our own replacement handle.
      if WINAPI_STDERR_HANDLE.is_null() {
        WINAPI_STDERR_HANDLE = std::io::stderr().as_raw_handle() as HANDLE;
      }

      if SetStdHandle(STD_ERROR_HANDLE, handle) == 0 {
        let err = io::Error::last_os_error();
        CloseHandle(handle);
        return Err(err);
      }
      if !WINAPI_INSTALLED_HANDLE.is_null() {
        CloseHandle(WINAPI_INSTALLED_HANDLE);
      }
      WINAPI_INSTALLED_HANDLE = handle;
    }
  }

  #[cfg(not(target_os = "windows"))]
  {
    use std::os::unix::io::AsRawFd;

    unsafe {
      // Save the real stderr fd (not stderr's *current* fd compared against
      // the target) so a later `restore_stderr` can put it back. Close any
      // previously-saved dup before overwriting.
      if UNIX_STDERR_HANDLE != -1 {
        libc::close(UNIX_STDERR_HANDLE);
      }
      UNIX_STDERR_HANDLE = libc::dup(libc::STDERR_FILENO);
      if UNIX_STDERR_HANDLE == -1 {
        return Err(io::Error::last_os_error());
      }

      // dup2 gives fd 2 its own reference, so the caller's `File` can be
      // dropped without affecting the redirect.
      if libc::dup2(target.as_raw_fd(), libc::STDERR_FILENO) == -1 {
        return Err(io::Error::last_os_error());
      }
    }
  }

  Ok(())
}

fn restore_stderr() -> io::Result<()> {
  #[cfg(target_os = "windows")]
  {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Console::{STD_ERROR_HANDLE, SetStdHandle};

    unsafe {
      if WINAPI_STDERR_HANDLE.is_null() {
        return Ok(());
      }
      if SetStdHandle(STD_ERROR_HANDLE, WINAPI_STDERR_HANDLE) == 0 {
        return Err(io::Error::last_os_error());
      }
      WINAPI_STDERR_HANDLE = std::ptr::null_mut();
      if !WINAPI_INSTALLED_HANDLE.is_null() {
        CloseHandle(WINAPI_INSTALLED_HANDLE);
        WINAPI_INSTALLED_HANDLE = std::ptr::null_mut();
      }
    }
  }

  #[cfg(not(target_os = "windows"))]
  {
    unsafe {
      if UNIX_STDERR_HANDLE == -1 {
        return Ok(());
      }
      if libc::dup2(UNIX_STDERR_HANDLE, libc::STDERR_FILENO) == -1 {
        return Err(io::Error::last_os_error());
      }
      libc::close(UNIX_STDERR_HANDLE);
      UNIX_STDERR_HANDLE = -1;
    }
  }

  Ok(())
}

#[cfg(all(test, unix))]
mod tests {
  use super::*;
  use std::io::Write;

  // One test only: it swaps the process's real fd 2, which any concurrently
  // running test in this binary would otherwise see.
  #[test]
  fn redirect_to_file_captures_stderr_and_restores_on_drop() {
    let dir = std::env::temp_dir()
      .join(format!("redirect-stderr-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("stderr.log");
    let file = File::create(&path).unwrap();

    let before = unsafe { libc::dup(libc::STDERR_FILENO) };
    {
      let _guard = redirect_stderr_to_file(&file).unwrap();
      drop(file);
      // Write straight to fd 2: libtest captures `eprintln!` itself, which
      // would hide whether the OS-level redirect works.
      let msg = b"from a C library\n";
      let n = unsafe {
        libc::write(libc::STDERR_FILENO, msg.as_ptr().cast(), msg.len())
      };
      assert_eq!(n, msg.len() as isize);
      let _ = io::stderr().flush();
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "from a C library\n");

    // fd 2 points at the same file as before the redirect again.
    let (mut a, mut b): (libc::stat, libc::stat) =
      unsafe { (std::mem::zeroed(), std::mem::zeroed()) };
    unsafe {
      libc::fstat(before, &mut a);
      libc::fstat(libc::STDERR_FILENO, &mut b);
      libc::close(before);
    }
    assert_eq!((a.st_dev, a.st_ino), (b.st_dev, b.st_ino));
    let _ = std::fs::remove_dir_all(&dir);
  }
}
