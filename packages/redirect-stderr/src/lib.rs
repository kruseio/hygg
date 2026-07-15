// #[cfg(target_family = "unix")]
// fn get_stdout() -> Result<RawFd, Error> {
//     use std::os::unix::io::AsRawFd;
//     use std::io::stdout;
//     Ok(stdout().as_raw_fd())
// }

// #[cfg(target_family = "windows")]
// fn get_stdout() -> Result<std::os::windows::io::RawHandle, dyn
// std::error::Error> {     use std::io::stdout;
//     Ok(stdout().as_raw_handle())
// }

// use std::os::fd::FromRawFd as _;
#[cfg(target_family = "windows")]
static mut WINAPI_STDERR_HANDLE: windows_sys::Win32::Foundation::HANDLE =
  std::ptr::null_mut();
#[cfg(not(target_os = "windows"))]
static mut UNIX_STDERR_HANDLE: i32 = -1;

#[cfg(target_family = "windows")]
static mut WINAPI_STDOUT_HANDLE: windows_sys::Win32::Foundation::HANDLE =
  std::ptr::null_mut();
#[cfg(not(target_os = "windows"))]
static mut UNIX_STDOUT_HANDLE: i32 = -1;

pub fn redirect_stderr() -> std::io::Result<()> {
  use std::fs::File;
  use std::io::{self};

  #[allow(unused_variables)]
  let dev_null = if cfg!(target_os = "windows") {
    File::create("NUL")?
  } else {
    File::create("/dev/null")?
  };

  #[cfg(target_os = "windows")]
  {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{
      HANDLE_FLAG_INHERIT, SetHandleInformation,
    };
    use windows_sys::Win32::System::Console::{STD_ERROR_HANDLE, SetStdHandle};

    unsafe {
      // Ensure the handle is not inherited
      let handle =
        dev_null.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
      SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);

      if (WINAPI_STDERR_HANDLE != handle) {
        WINAPI_STDERR_HANDLE = std::io::stdout().as_raw_handle()
          as windows_sys::Win32::Foundation::HANDLE;
      }

      // Redirect stderr to NUL
      if SetStdHandle(STD_ERROR_HANDLE, handle) == 0 {
        return Err(io::Error::last_os_error());
      }
    }
  }

  #[cfg(not(target_os = "windows"))]
  {
    use libc;
    use std::os::unix::io::AsRawFd;

    let dev_null_fd = dev_null.as_raw_fd();

    unsafe {
      // Save the real stderr fd (not stderr's *current* fd compared against
      // dev_null) so a later `restore_stderr` can put it back. Close any
      // previously-saved dup before overwriting.
      if UNIX_STDERR_HANDLE != -1 {
        libc::close(UNIX_STDERR_HANDLE);
      }
      UNIX_STDERR_HANDLE = libc::dup(libc::STDERR_FILENO);
      if UNIX_STDERR_HANDLE == -1 {
        return Err(io::Error::last_os_error());
      }

      if libc::dup2(dev_null_fd, libc::STDERR_FILENO) == -1 {
        return Err(io::Error::last_os_error());
      }
    }
  }

  Ok(())
}

pub fn restore_stderr() -> std::io::Result<()> {
  use std::io::{self};

  #[cfg(target_os = "windows")]
  {
    use windows_sys::Win32::System::Console::{STD_ERROR_HANDLE, SetStdHandle};

    unsafe {
      if SetStdHandle(STD_ERROR_HANDLE, WINAPI_STDERR_HANDLE) == 0 {
        return Err(io::Error::last_os_error());
      }
    }
  }

  #[cfg(not(target_os = "windows"))]
  {
    use libc;

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

pub fn redirect_stdout() -> std::io::Result<()> {
  use std::fs::File;
  use std::io::{self};

  #[allow(unused_variables)]
  let dev_null = if cfg!(target_os = "windows") {
    File::create("NUL")?
  } else {
    File::create("/dev/null")?
  };

  #[cfg(target_os = "windows")]
  {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{
      HANDLE_FLAG_INHERIT, SetHandleInformation,
    };
    use windows_sys::Win32::System::Console::{
      STD_OUTPUT_HANDLE, SetStdHandle,
    };

    unsafe {
      // Ensure the handle is not inherited
      let handle =
        dev_null.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
      SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);

      if (WINAPI_STDOUT_HANDLE != handle) {
        WINAPI_STDOUT_HANDLE = std::io::stdout().as_raw_handle()
          as windows_sys::Win32::Foundation::HANDLE;
      }

      // Redirect stderr to NUL
      if SetStdHandle(STD_OUTPUT_HANDLE, handle) == 0 {
        return Err(io::Error::last_os_error());
      }
    }
  }

  #[cfg(not(target_os = "windows"))]
  {
    use libc;
    use std::os::unix::io::AsRawFd;

    // Use the `dev_null` File bound above. A previous version opened
    // /dev/null again inline and then read `.as_raw_fd()` off the dropped
    // temporary, leaving the local `dev_null_fd` pointing at an already-
    // closed descriptor — the subsequent `dup2` then silently failed and
    // stdout was never actually redirected on Unix.
    let original_fd = io::stdout().as_raw_fd();
    let dev_null_fd = dev_null.as_raw_fd();

    unsafe {
      // Save the original stdout fd the first time we're called so a later
      // `restore_stdout` can put it back. Close the previously-saved dup if
      // we're stacking redirects, to avoid leaking fds.
      if UNIX_STDOUT_HANDLE != -1 {
        libc::close(UNIX_STDOUT_HANDLE);
      }
      UNIX_STDOUT_HANDLE = libc::dup(original_fd);
      if UNIX_STDOUT_HANDLE == -1 {
        return Err(io::Error::last_os_error());
      }

      if libc::dup2(dev_null_fd, original_fd) == -1 {
        return Err(io::Error::last_os_error());
      }
    }
  }

  Ok(())
}

pub fn restore_stdout() -> std::io::Result<()> {
  use std::io::{self};

  #[cfg(target_os = "windows")]
  {
    use windows_sys::Win32::System::Console::{
      STD_OUTPUT_HANDLE, SetStdHandle,
    };

    unsafe {
      if SetStdHandle(STD_OUTPUT_HANDLE, WINAPI_STDOUT_HANDLE) == 0 {
        return Err(io::Error::last_os_error());
      }
    }
  }

  #[cfg(not(target_os = "windows"))]
  {
    use libc;
    use std::os::unix::io::AsRawFd;

    let original_fd = io::stdout().as_raw_fd();

    unsafe {
      if UNIX_STDOUT_HANDLE == -1 {
        // Nothing to restore.
        return Ok(());
      }
      if libc::dup2(UNIX_STDOUT_HANDLE, original_fd) == -1 {
        return Err(io::Error::last_os_error());
      }
      // We're done with the saved fd; release it and reset the sentinel
      // so a follow-up `redirect_stdout` saves the (now restored) original
      // rather than overwriting the saved dup it still holds.
      libc::close(UNIX_STDOUT_HANDLE);
      UNIX_STDOUT_HANDLE = -1;
    }
  }

  Ok(())
}
