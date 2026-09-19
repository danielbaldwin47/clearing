# tui-disk

A Linux terminal disk explorer: see where the space went, open a folder, and delete an item with an explicit confirmation.

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
| `d` | Open the permanent deletion confirmation |
| `r` | Rescan, preserving the current directory when possible |
| `?` | Show help |
| Escape | Cancel a dialog or scan; stop an active deletion; otherwise quit |
| `q`, Control-C | Quit |

Deletion requires typing `delete` and pressing Enter. Escape cancels the confirmation. During deletion, a live count shows entries actually removed; Escape or Control-C stops before the next removal. Entries already removed stay removed. The selected item and its descendants are checked against their scanned device and inode numbers. Deletion uses directory file descriptors without following symlinks, refuses incomplete directory scans and filesystem crossings, and never exposes deletion of the current view itself. A changed entry stops deletion; already removed children cannot be restored. Every deletion attempt rescans the tree.

Sizes count allocated blocks, including directory blocks, rather than apparent file length. Sparse files therefore show the space actually allocated. Hard links count once within the scanned tree; another link may prevent deleted data from releasing space. The root path is resolved once; symlink targets discovered inside it are not scanned. Filesystem compression, snapshots, reflinks and open deleted files can make actual free-space changes differ from the displayed allocation. Inaccessible entries are reported and the screen marks partial results.

A true-color terminal with at least 60 columns and 20 rows is required; 120 × 40 or larger is recommended. It runs directly in terminals under Hyprland, with no graphical window or display-server dependency. Startup scans run in the background and can be cancelled. Raw mode and the alternate screen are restored on ordinary exit and errors.

To scan without a terminal:

```sh
./target/release/tui-disk --scan --json "$HOME"
```

The summary includes `path`, `bytes`, `apparent_bytes`, `files`, `directories`, `errors`, and `elapsed_seconds`. `--summary` aliases `--json`; both use the same full in-memory scan as the interactive app. The root counts as a directory. Paths starting with a hyphen can follow `--`. In displayed names and the summary path, control characters, backslashes, and invalid UTF-8 bytes use explicit escapes; filesystem operations always retain the original path bytes.

The independent judging and benchmarking evidence lives in `judging/`, `progress/`, and `benchmarks/`. `--snapshot overview|drilled|delete` renders deterministic ANSI for inspection; the official visual comparisons use actual terminal captures. The separate `--wireframe` renderer is a runnable minimal prototype for calibrating the visual comparisons.

```sh
cargo test
python scripts/acceptance.py
```
