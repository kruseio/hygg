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
    use windows_sys::Win32::System::Console::{
      STD_ERROR_HANDLE, SetStdHandle,
    };

    unsafe {
      // Ensure the handle is not inherited
      let handle = dev_null.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
      SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);

      if (WINAPI_STDERR_HANDLE != handle) {
        WINAPI_STDERR_HANDLE =
          std::io::stdout().as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
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

    unsafe {
      let raw_fd = dev_null.as_raw_fd();

      if (UNIX_STDERR_HANDLE != raw_fd) {
        UNIX_STDERR_HANDLE = std::io::stdout().as_raw_fd();
      }

      if libc::dup2(raw_fd, libc::STDERR_FILENO) == -1 {
        return Err(io::Error::last_os_error());
      }
    }
  }

  Ok(())
}

pub fn restore_stderr() -> std::io::Result<()> {
  use std::fs::File;
  use std::io::{self};

  #[cfg(target_os = "windows")]
  {
    use windows_sys::Win32::System::Console::{
      STD_ERROR_HANDLE, SetStdHandle,
    };

    unsafe {
      if SetStdHandle(STD_ERROR_HANDLE, WINAPI_STDERR_HANDLE) == 0 {
        return Err(io::Error::last_os_error());
      }
    }
  }

  #[cfg(not(target_os = "windows"))]
  {
    use libc;
    use std::os::unix::io::AsRawFd;

    unsafe {
      if libc::dup2(UNIX_STDERR_HANDLE, libc::STDERR_FILENO) == -1 {
        return Err(io::Error::last_os_error());
      }
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
      let handle = dev_null.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
      SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);

      if (WINAPI_STDOUT_HANDLE != handle) {
        WINAPI_STDOUT_HANDLE =
          std::io::stdout().as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
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

    // Save the original stdout
    let stdout = io::stdout();
    let original_fd = stdout.as_raw_fd();

    // Redirect stdout to /dev/null
    let dev_null_fd = File::open("/dev/null").unwrap().as_raw_fd();

    unsafe {
      if (UNIX_STDOUT_HANDLE != dev_null_fd) {
        UNIX_STDOUT_HANDLE = libc::dup(original_fd);
      }

      libc::dup2(dev_null_fd, original_fd);
    }

    // unsafe {
    //   let raw_fd = dev_null.as_raw_fd();

    //   if (UNIX_STDOUT_HANDLE != raw_fd) {
    //     UNIX_STDOUT_HANDLE = std::io::stdout().as_raw_fd();
    //   }

    //   if libc::dup2(raw_fd, libc::STDOUT_FILENO) == -1 {
    //     return Err(io::Error::last_os_error());
    //   }
    // }
  }

  Ok(())
}

pub fn restore_stdout() -> std::io::Result<()> {
  use std::fs::File;
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

    // Save the original stdout
    let stdout = io::stdout();
    let original_fd = stdout.as_raw_fd();

    unsafe {
      libc::dup2(UNIX_STDOUT_HANDLE, original_fd);
    }
  }

  Ok(())
}
