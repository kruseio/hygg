### [<-](../README.md)

## Reporting a bug
Open an issue at https://github.com/kruseio/hygg/issues and attach the files
below. They tell us the version, platform, terminal and input encoding, and for
a crash the exact location, which is usually enough to reproduce it.

### Reproduce with debug logging on
```sh
HYGG_DEBUG=1 hygg document.pdf
```
On exit hygg prints `hygg debug log: <path>`. The log records the hygg version,
OS and architecture, terminal size, `TERM`/`LANG` settings, and the input file
type and detected encoding, followed by what the reader did. It is overwritten
on each debug run, so attach it before running hygg again.

### If hygg crashed
hygg restores the terminal and prints:
```
hygg crashed: <message> at <file:line>. Details: <path>/crash-<time>-<pid>.log
```
Attach that crash file. It holds the panic message, its location and a
backtrace. With `HYGG_DEBUG=1` the full report is also printed to the terminal.

### Where the files are
| Platform | Folder |
| --- | --- |
| Linux | `~/.config/hygg/logs/` |
| macOS | `~/Library/Application Support/hygg/logs/` |
| Windows | `%APPDATA%\hygg\logs\` |

The folder holds `debug.log`, `crash-*.log`, and `stderr.log` (anything a
library printed while the reader was on screen). Set `HYGG_LOG_DIR` to use a
different folder.
