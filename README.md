# tui-disk

A terminal disk explorer for Linux and macOS: see where the space went, open a folder, and collect items across directories for one confirmed move to Trash.

```sh
cargo build --release
./target/release/tui-disk "$HOME"
```

The screen combines a treemap with a list ordered by disk usage. Rectangle area represents allocated bytes; nested rectangles preview the next directory level. Categories retain related colors as you drill down. Small entries are grouped into a remainder tile while every entry remains selectable in the list.

| Key | Action |
| --- | --- |
| Up / Down, `k` / `j` | Select an entry |
| Enter / Right | Open the selected directory |
| Backspace / Left | Return to its parent |
| Home / End | Select the first / last entry |
| Page Up / Page Down | Move eight entries |
| Space | Add or remove the selected item from the collector |
| `c` | Review the collector |
| `t` | Move the highlighted item to system Trash after confirmation |
| `d` | Open the permanent deletion confirmation |
| `r` | Rescan, preserving the current directory when possible |
| `?` | Show help |
| Escape | Close a dialog or collector; cancel a scan or stop a running operation; otherwise quit |
| `q`, Control-C | Quit |

Press Space on files or folders as you explore, then `c` to review the collection. The collection stays available across directory changes and rescans for the current session. Collecting a folder replaces collected descendants, so its contents are not counted or submitted twice. Items already covered by a collected folder are marked accordingly.

For one item, press `t` while browsing, type `trash`, and press Enter. This moves only the highlighted item; unrelated items in the collector stay queued. Escape cancels. Press `d` when you want the separate permanent-deletion action.

In the collector, use Up/Down or `j`/`k` to select a row, Space or Backspace to remove it, and Escape to return to browsing. Press `t`, type `trash`, and press Enter to move the collection to the desktop Trash. Escape cancels the confirmation. During a batch, Escape or Control-C stops after the current item; successful moves leave the collection, while failed and unprocessed items stay available. Rescanning does not authorize a replacement file at an old collected path.

On Linux, recoverable trashing uses `gio trash` from Arch's `glib2` package, following the [Freedesktop Trash specification](https://specifications.freedesktop.org/trash/latest/). On macOS, it uses the native `NSFileManager` Trash API through [trash-rs](https://github.com/Byron/trash-rs/tree/master/src/macos), with no `gio` dependency or Finder automation. Restore items by dragging them out of Trash; macOS may not offer **Put Back** for this API. Invalid UTF-8 paths are refused on macOS so the native backend cannot address a different filename. Missing services and unsupported filesystems produce errors, never a fallback to permanent deletion. **Moving items to Trash does not free their disk space; empty Trash when you are ready.** The collector does not empty Trash.

Deletion requires typing `delete` and pressing Enter. Escape cancels the confirmation. During deletion, a live count shows entries actually removed; Escape or Control-C stops before the next removal. Entries already removed stay removed. The selected item and its descendants are checked against their scanned device and inode numbers. Deletion uses directory file descriptors without following symlinks, refuses incomplete directory scans and filesystem crossings, and never exposes deletion of the current view itself. A changed entry stops deletion; already removed children cannot be restored. Every deletion attempt rescans the tree.

Sizes count allocated blocks, including directory blocks, rather than apparent file length. Sparse files therefore show the space actually allocated. Hard links count once within the scanned tree; another link may prevent deleted data from releasing space. The root path is resolved once; symlink targets discovered inside it are not scanned. Filesystem compression, snapshots, reflinks and open deleted files can make actual free-space changes differ from the displayed allocation. Inaccessible entries are reported and the screen marks partial results.

A true-color terminal with at least 60 columns and 20 rows is required; 120 × 40 or larger is recommended. It runs directly in terminals under Hyprland, with no graphical window or display-server dependency. Startup scans run in the background and can be cancelled. Raw mode and the alternate screen are restored on ordinary exit and errors.

On a Mac with a current Rust toolchain and Xcode Command Line Tools installed, use the same `cargo build --release --locked` command. Both Apple Silicon and Intel targets are supported by the build configuration. macOS privacy permissions still apply to protected folders. The GitHub Actions workflow builds and tests Linux and both Mac architectures; local checks made from Linux cannot prove interactive terminal or native Trash behavior on a Mac.

To scan without a terminal:

```sh
./target/release/tui-disk --scan --json "$HOME"
```

The summary includes `path`, `bytes`, `apparent_bytes`, `files`, `directories`, `errors`, and `elapsed_seconds`. `--summary` aliases `--json`; both use the same full in-memory scan as the interactive app. The root counts as a directory. Paths starting with a hyphen can follow `--`. In displayed names and the summary path, control characters, backslashes, and invalid UTF-8 bytes use explicit escapes; filesystem operations always retain the original path bytes.

The original release's independent judging and benchmarking evidence lives in `judging/`, `progress/`, and `benchmarks/`; those results belong to commit `176da86`, before the collector addition. `--snapshot overview|drilled|delete|collector` renders deterministic ANSI for inspection; the official visual comparisons use actual terminal captures. The separate `--wireframe` renderer is a runnable minimal prototype for calibrating the original visual comparisons.

```sh
cargo test
python scripts/acceptance.py
python scripts/collector_acceptance.py
```
