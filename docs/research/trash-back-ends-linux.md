# Trash back ends on Linux: what a native implementation must do

Research notes for issue #21 ("Trash back ends: what spacemap does itself"), Linux half.
They build on `docs/research/native-trash.md` (branch `research/native-trash`, sections
1.1–1.3), which already covers the text of the freedesktop Trash specification, what
`gio trash` does, and the nine gaps of the `trash` crate 5.2.9. Nothing from there is
repeated here except where a detail changes the answer.

Fixed before this pass: the app moves to Trash, lists the Trash with sizes, puts items
back and empties the Trash itself, on a headless machine with no session bus; emptying and
deleting never follow symbolic links. The lean is a fully native implementation of the
freedesktop layout, with no `gio` and no `trash` crate.

**VERIFIED** means read in the primary source or reproduced on this machine (Arch Linux,
kernel 7.2.3, btrfs root with `@`, `@home`, `@log`, `@pkg` subvolumes, GLib 2.88.3) while
writing this. **INFERRED** means concluded from those sources, not stated by them and not
tested.

**In brief.** No Rust crate qualifies, so the layout is written natively. The one rule
that prevents most failures is to predict `rename()` success before writing anything: the
item's parent and the trash's `files` directory must share both the mount ID and `st_dev`.
GLib gets this wrong in three reproducible ways (it compares against `$HOME`, not the
trash directory; it refuses every btrfs subvolume mount; it discovers `EXDEV` only after
writing the info file), and none of the three implementations is safe against symbolic
links when emptying. When a rename cannot work the recommendation is an honest refusal,
never a silent copy. `directorysizes` is tolerated and pruned, not trusted: KDE writes its
time field in milliseconds where the specification says seconds.

## Sources read

| Short name | What | Where |
|---|---|---|
| spec | Trash Specification 1.0 (2014-01-02) | <https://specifications.freedesktop.org/trash/latest/> |
| GLib | `gio/glocalfile.c`, `gio/gunixmounts.c`, `gio/gunixmounts-private.h`, `glib/guri.c`, `gio/gio-tool-trash.c`, branch `main`, read 2026-09-21 | <https://gitlab.gnome.org/GNOME/glib> |
| GVfs | `daemon/trashlib/{trashwatcher,trashdir,trashitem,trashexpunge}.c`, commit `86448383` | <https://gitlab.gnome.org/GNOME/gvfs> |
| KIO | `src/kioworkers/trash/{trashimpl,kio_trash,trashsizecache}.cpp`, branch `master`, read 2026-09-21 | <https://invent.kde.org/frameworks/kio> |
| trash-cli | `trashcli/{put,restore,empty,fstab,lib,fslib}`, commit `9295e9f4` | <https://github.com/andreafrancia/trash-cli> |
| man-pages | `rename(2)`, `open(2)`, `openat2(2)`, `unlink(2)`, `statx(2)`, `fsync(2)` | <https://git.kernel.org/pub/scm/docs/man-pages/man-pages.git> |
| overlayfs | kernel documentation | <https://docs.kernel.org/filesystems/overlayfs.html> |
| crates.io | registry API, read 2026-09-21 | <https://crates.io> |

---

## 1. Choosing the trash directory for a path

### 1.1 GLib (`g_local_file_trash`)

All **VERIFIED** in `gio/glocalfile.c` unless marked.

**Which device is compared.** GLib `lstat`s the item. For a directory it uses the item's own
`st_dev`; for anything else it `stat`s (following symbolic links) the item's *parent
directory* and uses that `st_dev`. The source gives both reasons: on overlayfs a file's
`st_dev` can differ from its directory's, and a parent that is a symbolic link to another
device would otherwise look like the home device.

**Home trash test.** `checked_st_dev == home_stat.st_dev`, where `home_stat` is
`g_stat(g_get_home_dir())`. The comparison is against **`$HOME`, never against
`$XDG_DATA_HOME`**. On a match the trash is `g_get_user_data_dir()/Trash`, created with
`g_mkdir_with_parents(trashdir, 0700)`.

**`$XDG_DATA_HOME` on a different device from `$HOME`.** GLib has no handling. A file on the
home device selects the home trash, the info file is written, `g_rename()` fails with
`EXDEV`, the info file is unlinked, and the user sees "Unable to trash file %s across
filesystem boundaries". A file on the `$XDG_DATA_HOME` device does *not* select the home
trash and goes to a volume trash instead. (The code path is VERIFIED; the scenario was not
reproduced, so the end-to-end outcome is INFERRED.)

**Finding the top directory.** Not the mount table: a pure `st_dev` walk.
`_g_local_file_find_topdir_for()` takes the item's parent with every symbolic link expanded
(`get_parent` → `expand_symlinks`, capped at `MAXSYMLINKS`/40), then `find_mountpoint_for()`
climbs while `parent_dev == dir_dev` and returns the last directory before the device
changes, or `/`.

**What it refuses.** After the walk, `ignore_trash_path(topdir)` looks the top directory up
in the mount table by **exact mount-path match** (`g_unix_mount_entry_at`, last matching
entry wins). The rules, in order:

1. No mount entry has that exact path → **refused**.
2. Mount options contain `x-gvfs-trash` → allowed. Contain `x-gvfs-notrash` → refused.
   Both are substring matches (`strstr`).
3. If the mount has no options, or is system-internal, the same two options are looked up
   in the `fstab` entry for that mount path (`g_unix_mount_point_at`). The source comment
   explains why: the `x-gvfs-*` options are userspace-only, never appear in
   `/proc/self/mountinfo`, and reach libmount only through `/run/mount/utab`, which
   mounts made by systemd, the initrd or an image-based OS never get.
4. Otherwise the answer is `g_unix_mount_entry_is_system_internal()`, which is
   `guess_system_internal()` in `gio/gunixmounts.c`. A mount is system-internal when any of
   these holds:
   - its filesystem type is in a fixed list that includes `autofs`, `overlay`, `tmpfs`,
     `ecryptfs`, `proc`, `sysfs`, `devtmpfs`, `cgroup2`, `fuse.gvfsd-fuse`, `fuse.portal`,
     `rootfs` and several cluster filesystems (`gfs2`, `ocfs2`, `lustre`, `gpfs`, `afs`);
   - its device path is one of `/dev/loop`, `/dev/vn`, `devpts`, `nfsd`, `none`, `sunrpc`
     (exact match, so `/dev/loop0` is not caught);
   - its mount path is in a fixed list (`gunixmounts-private.h`) that includes `/`, `/boot`,
     `/home`, `/usr`, `/var`, `/tmp`, `/opt`, `/srv`, `/mnt`, `/media`, `/root`, or starts
     with `/dev/`, `/proc/`, `/sys/`, or ends with `/.gvfs`;
   - **its mount root is not `/`**: every bind mount and every btrfs subvolume mount. The
     comment says this exists because such mounts "usually just duplicate other mounts".

The refusal is `G_IO_ERROR_NOT_SUPPORTED`, "Trashing on system internal mounts is not
supported". Reproduced here, **VERIFIED**:

```
$ gio trash /var/tmp/trashtest-f      # "/" is btrfs subvolume @, home is @home
gio: file:///var/tmp/trashtest-f: Trashing on system internal mounts is not supported
$ gio trash /tmp/trashtest-f          # tmpfs
gio: file:///tmp/trashtest-f: Trashing on system internal mounts is not supported
$ btrfs subvolume create sub && echo hi > sub/f && gio trash sub/f   # nested subvolume in ~
gio: file:///home/.../sub/f: Trashing on system internal mounts is not supported
```

The third case is rule 1: a nested subvolume has its own `st_dev` (72 against 63 for the
directory holding it) and no mount-table entry, so the walk stops at a directory the mount
table does not know. Today's `gio trash -- PATH` executor has all three refusals.

**Choosing between the two volume layouts.** `$topdir/.Trash` is used when `lstat` says it
is a directory with `S_ISVTX`; `lstat` plus `S_ISDIR` is also the not-a-symlink check. Then
`$topdir/.Trash/$uid` must, if it exists, be a directory owned by the effective uid,
otherwise it is ignored and GLib falls through to `$topdir/.Trash-$uid`. When created,
`.Trash-$uid` is re-`lstat`ed to confirm the owner really is the user ("This might fail on
e.g. a FAT dir"); if not, the new directory is removed and trashing fails with "Unable to
find or create trash directory". GLib never falls back to the home trash from a volume.

**Pre-flight on directories.** For a directory the user owns, `check_removing_recursively()`
walks it without following symbolic links and refuses ("Unable to trash child file %s") if
any entry sits inside a subdirectory owned by someone else, because the eventual erase
would fail.

### 1.2 KDE (`TrashImpl::findTrashDirectory`)

**VERIFIED** in `trashimpl.cpp`.

- Uses the **mount table**, not an `st_dev` walk: `KMountPoint::currentMountPointForPath(origPath)`.
- Home trash when the mount's device ID equals the device ID of the mount holding
  `QDir::homePath()`. Like GLib, this compares against `$HOME`, not `$XDG_DATA_HOME`.
- Refuses pseudo filesystems that are not encrypted ones (`isPseudoFs() && !isEncryptedFs()`).
  No mount-path list and no mount-root rule, so btrfs subvolume mounts are accepted.
- `.Trash` must be a directory, not a symbolic link, sticky, **and** pass `access(W_OK)`;
  `.Trash/$uid` and `.Trash-$uid` must pass `access(W_OK|X_OK)`. The comment: "we use
  access() because checking mode isn't sufficient for remote directories".
- If `.Trash` or `.Trash-$uid` turns out to be on the home device ("bind mount, maybe"),
  the home trash is used instead.
- No home-trash fallback when a volume trash cannot be made: `// TODO Add fallback to home
  trash if settings allow it`, then `ERR_TRASH_NOT_AVAILABLE`.
- Before moving, `adaptTrashSize()` can refuse or evict old items under the user's
  trash-size limit. A native implementation has no such setting; noted only because a KDE
  user may see different behaviour between the two.

### 1.3 trash-cli (`trash-put`)

**VERIFIED** in `put/trash_directories_finder.py`, `put/same_volume_gate.py`,
`fstab/volume_of_impl.py`, `lib/trash_dirs.py`.

- The item's volume is the volume of the **real path of its parent**, found by climbing
  until `os.path.ismount()` is true. `ismount` is "different `st_dev` from the parent, or
  same inode as the parent", so a nested btrfs subvolume counts as a volume (reproduced:
  `os.path.ismount(sub)` is `True`) and a bind mount of the same filesystem does not
  (reproduced: `False`).
- Candidates are tried in order: home trash, `$volume/.Trash/$uid`, `$volume/.Trash-$uid`,
  and last a home-trash fallback that is off unless `TRASH_ENABLE_HOME_FALLBACK=1` or the
  hidden `--home-fallback` flag is given.
- **The home-trash gate compares the volume of the trash directory itself**
  (`$XDG_DATA_HOME/Trash`) with the item's volume. This is the only one of the three that
  handles `$XDG_DATA_HOME` on another device correctly.
- Refuses nothing by filesystem type when putting. `.Trash` must exist, be a directory, not
  a symbolic link and sticky; in every candidate, `info` and `files` must not be symbolic
  links.

### 1.4 What to copy

| Behaviour | Copy from | Why |
|---|---|---|
| Decide by the item's **parent**, symbolic links resolved | GLib, trash-cli | The item may itself be a symbolic link or a mount point; the parent is what `rename()` cares about |
| Compare against **the trash directory**, not `$HOME` | trash-cli | The only correct answer when `$XDG_DATA_HOME` is elsewhere |
| Top directory from the **mount table** | KDE | spacemap already parses `/proc/self/mountinfo`; an `st_dev` walk invents top directories (nested subvolumes) that no lister will ever find |
| Own-uid check on `.Trash/$uid`, re-`lstat` after creating `.Trash-$uid` | GLib | Catches FAT, NTFS and uid-mapped network mounts without a filesystem-type list |
| `info` and `files` must be real directories | trash-cli | Closes the symbolic-link redirect the specification does not mention |
| Pre-flight ownership walk of a directory | GLib | Prevents a Trash item that can never be emptied |
| Filesystem-type refusals | GLib's type list | Trashing on `tmpfs` frees nothing; `autofs`, `proc`, `overlay` roots are not places for a trash |
| Mount-path list and the mount-root rule | **do not copy** | They refuse `/data` mounted with `subvol=@data` and every file on `/` of a subvolume-rooted install, which is the default Arch/openSUSE/Fedora btrfs layout |

One mechanism does both the `st_dev` and the mount-table job. `statx(2)` with
`STATX_MNT_ID` (Linux 5.8) returns "the mount ID of the mount containing the file ... the
number in the first field in one of the records in `/proc/self/mountinfo`" (**VERIFIED**,
`statx(2)`). The mount point in that record (field 5) is the top directory, with no
prefix-matching and no ambiguity from over-mounts.

---

## 2. `EXDEV` inside what looks like one filesystem

`rename(2)`: "`EXDEV` *oldpath* and *newpath* are not on the same mounted filesystem.
(Linux permits a filesystem to be mounted at multiple points, but `rename()` does not work
across different mount points, even if the same filesystem is mounted on both.)"
**VERIFIED.**

### 2.1 Reproduced on this machine

All **VERIFIED**, unprivileged: `btrfs subvolume create` in `~/.cache`, and
`unshare -Urm` for the bind mount and the overlay.

| Case | `st_dev` | In `mountinfo`? | `rename()` into the sibling/parent |
|---|---|---|---|
| Nested btrfs subvolume, not mounted | **differs** (72 vs 63) | no | `EXDEV`, file and directory alike |
| Two subvolumes of one btrfs, each mounted (`/home` = `@home`, `/` = `@`) | differs (63 vs 32) | yes, roots `/@home` and `/@` | `EXDEV` |
| Bind mount of a directory onto its sibling | **same** (63 = 63) | yes, root is not `/` | `EXDEV` |
| overlayfs, both layers on one filesystem: lower-layer **file** | uniform (93 everywhere) | yes, type `overlay` | succeeds (copy-up) |
| overlayfs: lower-layer **directory** | uniform | yes | **`EXDEV`** |
| overlayfs: directory that exists only in the upper layer | uniform | yes | succeeds |

The overlayfs rows match the kernel documentation: renaming a lower or merged directory
will "return EXDEV error ... This is the default behavior", unless `redirect_dir` is on, in
which case "the directory will be copied up (but not the contents)" (**VERIFIED**,
overlayfs documentation). `redirect_dir=on` could not be tested: the unprivileged overlay
mount with `userxattr` rejected it (mount exit 32). Whether a given system has it on is not
discoverable from the type alone; `redirect_dir=` appears in the mount options in
`mountinfo` (INFERRED from the documentation's option list, not observed here). With
layers on different filesystems and no `xino`, non-directories report a different `st_dev`
from their directory (overlayfs documentation, inode-properties table), which is the case
GLib's parent-`stat` comment is about.

The rule that covers every row: **a rename is predictable only when source parent and
destination directory share the mount ID *and* `st_dev`**, and even then an overlay
lower-layer directory still fails. The first half is VERIFIED per row; its sufficiency for
filesystems not tested is INFERRED.

### 2.2 Network and FUSE mounts

None of this was reproducible here (the NAS mounts are idle `autofs` points), so every
behavioural claim is **INFERRED** except the quoted ones.

- **`O_EXCL`.** "On NFS, `O_EXCL` is supported only when using NFSv3 or later on kernel 2.6
  or later" (**VERIFIED**, `open(2)`). NFSv2 is not a practical concern. The specification
  only promises atomicity "at least on the same machine"; two clients trashing the same
  name into one NFS trash is covered by NFSv3+ exclusive create.
- **Ownership and the sticky bit.** With `root_squash`, `all_squash`, idmapping, CIFS
  `uid=`/`forceuid`, or sshfs without `default_permissions`, the uid the client sees on
  `.Trash-$uid` may not be the process's uid, and mode bits may be synthetic. `open(2)`
  itself warns that on uid-mapped NFS a successful `open()` can be followed by denied
  reads (**VERIFIED**). This is exactly why GLib re-`lstat`s after `mkdir` and KDE uses
  `access()` instead of mode bits. A trash directory that fails the owner check is
  unusable, and the honest result is a refusal naming the directory.
- **CIFS/FAT/exFAT/NTFS** cannot hold the sticky bit or per-user ownership in general, so
  `$topdir/.Trash` never qualifies and `.Trash-$uid` depends on the mount's `uid=` option.
- **`EXDEV` inside one network mount** happens on NFS when the export contains several
  server filesystems (each appears as its own submount with its own `st_dev`) and on FUSE
  unions such as mergerfs when source and destination land on different branches.

### 2.3 What each implementation does on `EXDEV`

| | On `EXDEV` | Source |
|---|---|---|
| GLib | Unlinks the info file, reports "Unable to trash file %s across filesystem boundaries". **Never copies.** The comment names the cause: "This can happen when the same device is mounted multiple times, or with bind mounts of the same fs." | `glocalfile.c`, VERIFIED |
| KDE | `directRename` maps `EXDEV` to `ERR_UNSUPPORTED_ACTION`, and `move()` then runs a full `KIO::moveAs` **copy-and-delete job**. On failure it deletes the partial copy and the info file. | `trashimpl.cpp`, `kio_trash.cpp`, VERIFIED |
| trash-cli | `shutil.move`: `os.rename`, and on any `OSError` **copy then delete**. On failure it removes the partial copy and the info file. | `fslib/fs_operations.py`, `put/janitor_tools/put_trash_dir.py`, VERIFIED |

### 2.4 Recommendation: refuse, do not copy

Refuse with an honest message, and detect the refusal *before* writing the info file
(mount ID plus `st_dev`), keeping `EXDEV` from `rename()` as the backstop for the overlay
case. Reasons, all INFERRED judgement from the facts above:

1. spacemap exists to free space. A copy fallback needs the item's size free *again*, on
   the volume or on the home disk, at the moment the user has the least of it.
2. A copy of a large directory is minutes of I/O behind a key press that everywhere else is
   instant, and it is not atomic: a crash leaves two partial trees.
3. The copy is lossy (ownership, hard links, xattrs, sparse files, reflinks), which the
   specification itself concedes, and the `trash` crate's copy was already counted as a
   gap for that reason.
4. GLib, the back end users have today, never copies, so refusing is not a regression.

The refusal must say what happened and what is still possible, for example: "`<name>` is
on `<topdir>`, where no Trash can be used (`<reason>`). Nothing was moved." Reasons worth
distinguishing: no writable trash directory on the volume; nested btrfs subvolume; bind
mount; overlay lower directory; ownership check failed. The specification allows this:
refuse, warn clearly, and "MUST NOT erase the file without user confirmation".

---

## 3. Writing the info file

### 3.1 Percent-encoding

All three writers agree on the output, **VERIFIED** for GLib and trash-cli, INFERRED for
the exact Qt reserved set:

- GLib: `g_uri_escape_string(path, "/", FALSE)`. In `glib/guri.c`, `_uri_encoder` with
  `allow_utf8 = FALSE` copies a byte only if it is unreserved (`A–Z a–z 0–9 - . _ ~`) or in
  the allowed string (`/`); every other byte becomes `%XX` with **uppercase** hex. Space
  becomes `%20`, `=` becomes `%3D`, and each byte of a multi-byte UTF-8 character is escaped
  separately. A non-UTF-8 name needs no special case: its raw bytes are escaped like any
  other.
- KDE: `QUrl::toPercentEncoding(origPath, "/")`. It starts from a `QString`, so a name that
  is not valid in the locale encoding may not round-trip (INFERRED).
- trash-cli: `url_quote(path, '/')`, same unreserved set.

Reading must be more liberal than writing: accept lowercase hex, accept unescaped bytes
above `0x7F`, leave a malformed `%` sequence as literal bytes, and never decode to a Rust
`String`; the decoded value is an `OsString` built from bytes.

Parsing, from the specification (**VERIFIED**): first line must be `[Trash Info]`; take the
**first** `Path=` and the first `DeletionDate=`; ignore every other line. Split on the
first `=` only. The `trash` crate's `split('=')` with `unwrap()` is the counter-example.

### 3.2 Relative or absolute `Path=`

Specification (**VERIFIED**): relative paths are "from the directory in which the trash
directory resides", "MUST not include a `..`", and the system "SHOULD support absolute
pathnames only in the 'home trash'". GLib writes the absolute path in the home trash and
`try_make_relative(filename, topdir)` in a volume trash, which expands every symbolic link
in both paths first and **falls back to the absolute path** if the item is not under the
top directory. KDE does the same through `makeRelativePath`. On read, KDE and GVfs both
accept an absolute path in a volume trash and prepend the top directory only to a relative
one (**VERIFIED**, `readInfoFile` and `trash_item_get_trashinfo`). The `trash` crate
writes absolute paths everywhere (gap 4), so they exist in the wild.

### 3.3 `DeletionDate`

Specification: "`YYYY-MM-DDThh:mm:ss` ... The time zone should be the user's (or
filesystem's) local time", and its own example is `20040831T22:32:08`, the basic date form.
GLib formats `"%Y-%m-%dT%H:%M:%S"` from `g_date_time_new_now_local()` (and writes
`9999-12-31T23:59:59` if it cannot get the time); trash-cli uses the same `strftime`
pattern; KDE writes `QDateTime::currentDateTime().toString(Qt::ISODate)`. All **VERIFIED**.
No writer adds an offset or `Z` (INFERRED for Qt). A reader accepts both `YYYY-MM-DD` and
`YYYYMMDD` date parts, tolerates a trailing `Z`, offset or fractional seconds by ignoring
them, and treats a missing or unparseable date as "unknown", never as an error: the `trash`
crate without `chrono` writes none at all (gap 3).

### 3.4 Names, collisions and length

The specification leaves trash names to the implementation and forbids deriving the
original name from them. Observed strategies, **VERIFIED**:

| | Second item called `foo.tar.gz` | Over-long names |
|---|---|---|
| GLib | `foo.2.tar.gz` (counter before the **first** dot, from 2) | On `ENAMETOOLONG`, cut `strlen(".trashinfo")` = 10 bytes off the **front**, on a UTF-8 character boundary, restart the counter; cut 7 more for its `.XXXXXX` temporary file if that is what overflowed |
| KDE | `KFileUtils::suggestName` | not handled: `ERR_CANNOT_WRITE` |
| trash-cli | `foo.tar.gz_1` … `_99`, then `_<random 0–65535>` | On `ENAMETOOLONG`, cut from the **end** by the length of suffix plus `.trashinfo` |

Reproduced (**VERIFIED**): on btrfs a 255-byte name is legal and the same name plus
`.trashinfo` fails with `ENAMETOOLONG`. The budget for a trash name is therefore
`NAME_MAX − 10` bytes including any collision suffix, 245 on most filesystems, but the limit
is per filesystem (eCryptfs is lower), so the robust rule is GLib's: react to
`ENAMETOOLONG` and shorten, do not hard-code 255.

### 3.5 Ordering, `O_EXCL`, and a rename that fails afterwards

Specification: the info file "MUST" be created first, atomically, with `O_EXCL`. All three
do that. Differences that matter, **VERIFIED**:

- GLib creates the empty file with `O_CREAT|O_EXCL` to claim the name, then writes the
  content with `g_file_set_contents_full(..., CONSISTENT | ONLY_EXISTING, 0600)`, which is a
  temporary file, `fsync`, and an atomic rename over the claimed name. The comment: "Write
  the full content of the info file before trashing to make sure someone doesn't read an
  empty file. See #749314".
- KDE and trash-cli write straight into the `O_EXCL` descriptor, mode `0600`, no `fsync`.
- trash-cli also skips a candidate name when `files/<name>` already exists, which protects
  an orphan in `files/` from being overwritten by the later `rename()`. GLib and KDE do not
  check, and `rename()` silently replaces an orphan file or empty directory of that name.
- When the move fails, all three unlink the info file they just created; KDE and trash-cli
  also remove a partial copy. None leaves the info file behind on purpose.

`renameat2(RENAME_NOREPLACE)` fails with `EEXIST` instead of replacing, and "requires
support from the underlying filesystem" (ext4 since 3.15; btrfs, tmpfs and cifs since
3.17; xfs since 4.0; many more since 4.9; an unsupported filesystem returns `EINVAL`)
(**VERIFIED**, `rename(2)`). It closes the
orphan-overwrite hole without a separate check.

### 3.6 Is `fsync` warranted?

Only GLib syncs, and only the info file; nobody syncs the directories. `fsync(2)`
(**VERIFIED**): syncing a file "does not necessarily ensure that the entry in the directory
containing the file has also reached disk. For that an explicit `fsync()` on a file
descriptor for the directory is also needed."

Judgement (INFERRED): the failure being guarded against is a power cut that leaves
`files/<name>` with an empty or missing info file, which is an item that can be emptied but
never put back. One `fsync` of a 100-byte file per trashed item, before the rename, is
cheap against the cost of the scan that preceded it and matches what users of `gio trash`
get today. Syncing `info/` and `files/` directories as well is not warranted: no other
implementation does, the ordering already guarantees that the worst crash outcome is an
info file without a payload (harmless, see section 5), and a batch of thousands of items
would pay two directory syncs each.

---

## 4. `directorysizes`

**Format** (specification, **VERIFIED**): one line per trashed *directory*,
`[size] [mtime] [percent-encoded-directory-name]`. Size is disk usage in bytes "in the same
way as the `du -B1` command calculates". `mtime` is the modification time **of the
`.trashinfo` file**, "not the modification time of the directory itself", as "the number of
seconds since Epoch". The name is a direct child of `files/`; `/` is not allowed even as
`%2F`. Only newline and `%` strictly need encoding, but a reader "MUST be able to read
names encoded" more fully. Plain files are never listed.

**Update protocol** (**VERIFIED**): "implementations MUST use a temporary file followed by
an atomic `rename()` operation ... The fact that the changes from one of the writers could
get lost isn't an issue, as the cache can be updated again later on."

**Who writes and reads it** (**VERIFIED**):

- KDE only. `TrashSizeCache` adds a line after moving a directory to the trash, removes it
  on delete and restore, renames it on rename, **deletes the whole file on empty**, and
  uses it when totalling the trash, all through `QSaveFile` (temporary file plus rename).
- GLib and GVfs: no occurrence of the string (established in `native-trash.md`).
- trash-cli: no occurrence of `directorysizes` in the tree at `9295e9f4`.
- `junkyard` (section 8) writes it; the `trash` crate does not.

**KDE deviates from the specification.** It stores
`trashInfo->lastModified().toMSecsSinceEpoch()`, **milliseconds**, and validates an entry
by comparing against the same millisecond value. A specification-conforming writer's
seconds never equal KDE's milliseconds, so each side treats the other's lines as stale:
KDE recomputes and appends its own, a conforming reader recomputes. Harmless but it means
the cache is only ever valid for the implementation that wrote it. **VERIFIED** in
`trashsizecache.cpp`; the interoperability consequence is INFERRED.

**Recommendation.** Already fixed: sizes shown come from walking. Therefore:

- **Never read it for display.**
- **Do not add entries.** spacemap has no reader that benefits, the only other reader (KDE)
  will not accept a conforming `mtime`, and writing KDE's milliseconds would knowingly
  violate the specification.
- **Do remove entries** for a directory spacemap puts back or empties, with a temporary
  file in the trash directory (`O_EXCL`, unique name) and `rename()`; when emptying the
  whole trash, delete the file as KDE does. A stale line is otherwise only cleaned up by
  KDE's next full total, and a later trashed directory that reuses the name with an info
  file of identical mtime would be mis-sized by KDE. Low probability; the removal is a few
  lines. Treat every failure here as ignorable.
- Tolerate any content: unparseable lines are copied through untouched or dropped, never
  an error.

---

## 5. Put back

The specification defines no restore procedure (`native-trash.md` 1.1). **VERIFIED**
behaviour:

| Case | trash-cli `trash-restore` | KDE `TrashProtocol::restore` | GVfs / `gio trash --restore` |
|---|---|---|---|
| Something exists at the original path | Refuses: "Refusing to overwrite existing file". `lexists`, so a dangling symbolic link counts. `--overwrite` allows replacing a non-directory only | `ERR_FILE_ALREADY_EXIST` (`overwrite = false`) | `g_file_move` without `OVERWRITE` fails with "exists"; `--force` overwrites |
| Parent directory is gone | **Creates it** (`mkdirs`) | **Refuses**: "The directory %1 does not exist anymore ... You can either recreate that directory and use the restore operation again, or drag the item anywhere else" | **Creates it** (`g_file_make_directory_with_parents`) |
| Original path is on another device | `shutil.move` copies and deletes | `move()` falls back to a KIO copy job | Inside the daemon, `trash_item_restore` uses `G_FILE_COPY_NO_FALLBACK_FOR_MOVE`: rename only |
| After success | Removes the info file | `deleteInfo`, drops the `directorysizes` line | Removes the info file |

None checks the restored path atomically; all are check-then-rename.

**Orphans.**

- *Info file without payload.* KDE and trash-cli enumerate `info/`, so they list it; KDE's
  empty treats "does not exist" as success and removes the info file; `trash-restore` fails
  at the move. GVfs enumerates `files/` (`trashdir.c`), so it never sees it. This state is
  also **transient in normal operation**: every writer creates the info file before the
  rename, and GVfs removes the payload before the info file when restoring or deleting.
- *Payload without info file.* GVfs lists it with no original path, so it can be deleted
  but not restored. KDE and trash-cli do not list it; both remove it on empty. KDE's orphan
  sweep uses `QFile::remove`, which cannot remove a non-empty directory, so orphan
  directories survive a KDE empty (code VERIFIED, consequence INFERRED). The specification
  calls this "an emergency case, and MUST be clearly presented as such to the user". No
  writer produces this state transiently, so it is safe to show at once.
- *GVfs `expunged/`.* GVfs deletes an item by renaming it to `$trash/expunged/<random>`,
  removing the info file, and erasing `expunged/` on a background thread
  (`trash_item_delete`, `trashexpunge.c`). Nothing sweeps the directory at start-up; it is
  only revisited by the next delete. A killed `gvfsd-trash` therefore leaves bytes that no
  implementation lists. **VERIFIED.** For a disk-usage tool this is a third place where
  Trash space hides.

---

## 6. Emptying

**Order** (**VERIFIED**). KDE removes the payload first and the info file only if that
succeeded or the payload was already gone, and says why: "We need to ensure that the
.trashinfo file is only removed when the corresponding files could indeed be removed
(#116371)". trash-cli yields the payload path then the info path as two independent
removals, so a failed payload removal still loses the info file and manufactures an orphan.
GVfs renames into `expunged/` first. KDE's order is the correct one: the two crash outcomes
are "info file without payload" (zero bytes, self-healing) against "payload without info
file" (space still used, unrestorable, invisible to KDE and trash-cli).

**Undeletable entries** (**VERIFIED** unless marked).

- *Directories without owner write/execute permission.* KDE runs a recursive
  `chmod u+w` before deleting ("otherwise we won't be able to delete files in them
  (#130780)"); GVfs sets each directory to `0700` as it descends; trash-cli relies on
  `shutil.rmtree` and reports "cannot remove".
- *Entries owned by another user inside a trashed directory.* Not removable; KDE keeps the
  info file and reports the last error. GLib's pre-flight walk exists to stop these being
  trashed in the first place.
- *Immutable or append-only files.* `unlink(2)`: "`EPERM` The file to be unlinked is marked
  immutable or append-only." Clearing the flag needs `CAP_LINUX_IMMUTABLE`, so no
  implementation can help; none tries.
- *Read-only volume.* `EROFS`. KDE maps it to `ERR_CANNOT_DELETE`.

**Symbolic links.** None of the three uses descriptor-relative removal; all build paths and
recurse. GVfs and GLib pass `NOFOLLOW_SYMLINKS` to queries, which protects against a
symbolic link *inside* the payload but not against a directory swapped for a symbolic link
between the check and the descent. The already-decided `openat`/`unlinkat` rule is stricter
than all prior art. The pieces (**VERIFIED**, man-pages): `open(2)` `O_NOFOLLOW` fails with
`ELOOP` when the last component is a symbolic link; `O_DIRECTORY` rejects non-directories;
`unlinkat(dirfd, name, AT_REMOVEDIR)` is `rmdir` relative to a descriptor;
`openat2(2)` `RESOLVE_NO_SYMLINKS | RESOLVE_BENEATH` (Linux 5.6) refuses symbolic links in
every component and any escape from `dirfd`, and `RESOLVE_NO_XDEV` refuses crossing a mount
point, which also stops an empty from descending into something mounted inside a trashed
directory.

**Unmounted and read-only volumes.** A trash on an unmounted volume does not exist as far
as the running system is concerned; KDE rescans the mount table at every listing
("This allows noticing plugged-in [e.g. removable] devices, or new mounts") and GVfs
follows the mount monitor. Nothing records trashes that went away. A read-only volume's
trash is listable and not emptiable.

**Races with a file manager.** INFERRED from the protocols above; nothing in the
specification provides a lock.

- A file manager trashing while spacemap empties: spacemap's snapshot of `info/` does not
  contain the new item, so it is untouched, provided spacemap empties **the items it
  listed** and not "everything in `files/`".
- A file manager restoring an item spacemap is emptying: the payload rename wins or the
  first `unlinkat` wins. `ENOENT` at any point is success, not an error.
- A file manager emptying at the same time: every `ENOENT` is success.
- The orphan sweep is the dangerous part: an info file whose payload has not arrived yet
  looks exactly like a stale one. Age is the only discriminator available.
- GVfs holds items in memory and learns of outside changes through file monitoring (or, on
  NFS/CIFS, only when watched: `decide_watch_type`), so a GNOME window may show an item for
  a moment after spacemap removed it. Harmless; GVfs's own operations then fail with "not
  found".

---

## 7. Discovery for listing

**What the others enumerate** (**VERIFIED**).

- KDE: every entry of the mount table that is not a non-encrypted pseudo filesystem, plus
  the home trash; full sticky/symlink/`access()` checks per top directory, rescanned on
  every listing.
- GVfs: every mount-table entry that `ignore_trash_mount()` does not refuse (same rules as
  section 1.1), plus the home trash; per mount both `.Trash/$uid/files` and
  `.Trash-$uid/files`. Its existence test (`dir_exists`) `lstat`s each path component and
  requires a real directory, so symbolic links are refused, but **there is no sticky-bit
  check anywhere in `trashlib`** (no `S_ISVTX` in the directory). It also skips mount
  points without read access "to avoid polling".
- trash-cli: `psutil.disk_partitions(all=True)` filtered to physical filesystem types plus
  a hand-kept allow-list (`nfs`, `nfs4`, `p9`, `btrfs`, `fuse`, `fuse.glusterfs`,
  `fuse.mergerfs`, `fuse.gocryptfs`) and `tmpfs` on `/tmp`; `TRASH_VOLUMES` overrides it.
  For reading, `.Trash/$uid` requires the sticky, non-symlink parent, and any trash whose
  `info` or `files` is a symbolic link **or world-writable** is skipped with a warning.

The specification's MUSTs for listing: check `.Trash` "in all top directories that are
known", apply the same sticky and not-a-symlink checks as for trashing ("MUST NOT use this
directory for either trashing or undeleting"), and SHOULD list both layouts.

**Cost and safety.** Reading `/proc/self/mountinfo` never touches a mounted filesystem, so
it cannot hang (INFERRED; it is a procfs read of kernel state). Every `lstat` under a
network mount can block without limit if the server is gone, and a thread blocked in
uninterruptible sleep cannot be cancelled. `statx(2)` offers two mitigations
(**VERIFIED**): `AT_NO_AUTOMOUNT`, "Don't automount the terminal component ... can be used
in tools that scan directories to prevent mass-automounting", and `AT_STATX_DONT_SYNC`,
"on a network filesystem, it may not involve a round trip to the server". Neither is a
guarantee against a dead server (INFERRED).

This machine shows the `autofs` case concretely (**VERIFIED**): four
`/mnt/truenas/*` entries of type `autofs` with nothing mounted behind them. An `lstat` of
`/mnt/truenas/media/.Trash-1000` would trigger the automount and wait on the NAS. GLib's
type list contains `autofs`, so GVfs never probes them; once a share is really mounted, a
second `mountinfo` record of type `nfs4` or `cifs` appears on the same mount point and is
probed like any other.

---

## 8. Rust crates other than `trash`

Searched the crates.io API on 2026-09-21 for `trash`, `freedesktop trash`, `xdg trash`,
`trashcan`, `recycle bin`, `trash-cli`, `rm trash`, then read metadata and dependencies of
every hit that offers a library. `gtrash` is a Go program, not a crate. Licence of this
repository: MIT, `rust-version = "1.88"`. All **VERIFIED** from the registry or the
published source unless marked.

**`junkyard` 0.1.0** (MIT OR Apache-2.0; created 2026-05-26, last release 2026-06-13, 234
downloads). The only serious new implementation: `rustix`, parses `/proc/self/mountinfo`
instead of `getmntent`, checks the sticky bit and symbolic links on `.Trash`, makes `Path=`
relative to the mount point, writes `directorysizes` through a temporary file. It fixes
gaps 4 and 7 of the `trash` crate. But its whole public API is `discard()` and
`discard_all()`: **no list, no restore, no empty**, and no `EXDEV` handling was found in
its Linux module. Three months old, one author, no dependants. Does not qualify; worth
reading as a second opinion when writing the mount and permission code.

**`garbage` 0.4.3 / `garbage-fs` 0.1.0** (GPL-3.0-only; last release 2024-05-29, sourcehut).
An independent freedesktop implementation with put, list, restore and empty behind a CLI,
with a small library. **GPL-3.0-only cannot be linked into an MIT binary** without
relicensing the result. Disqualified on licence alone; not read further.

**`trash_lib` 0.1.0** (MIT/Apache-2.0; single release 2020-10-14, no repository link) and
**`trash-utils` 0.2.0** (MIT; last release 2020-08-31). Both are early independent
implementations, abandoned for six years, both predate or ignore `directorysizes`, and
`trash_lib` uses `fs_extra` for copying. Do not qualify on maintenance.

**`trash-core` 0.3.2** (Apache-2.0; 2026-08-08). A forensic, **read-only** parser
(`parse_trashinfo`, `scan_trash`) that takes a trash directory as input. No discovery, no
writing, no sizes. Could not replace more than the info-file parser, which is about forty
lines.

**`trash-cli-core` 0.1.0** (GPL-3.0-or-later; 2026-03-01, no repository, 24 downloads).
Licence and maturity both disqualify it.

**Everything else** that turned up (`conceal`, `trashy`, `rtrash`, `rmxt`, `file-handle`,
`trash-rs-cli`) depends on the `trash` crate and inherits its gaps; `trasher` uses its own
non-freedesktop store.

**Plainly: none qualifies.** No crate offers list-with-sizes, restore and empty on the
freedesktop layout with a compatible licence and live maintenance, other than `trash`
itself. The native route stands.

---

## What this means for the decision

Each line is meant to become an acceptance criterion. "Refuse" always means: nothing on
disk changed, and the message names the item, the place and the reason.

### A. Choosing the trash directory

1. Work from the item's **parent directory, canonicalised**; the item itself is never
   followed (it may be a symbolic link).
2. Get the parent's mount ID and `st_dev` (`statx` with `STATX_MNT_ID`; on a kernel older
   than 5.8, longest-prefix match in `mountinfo`, last record wins). The mount point of
   that record is the **top directory**.
3. **Home trash** is `$XDG_DATA_HOME/Trash` (default `~/.local/share/Trash`), created
   `0700` with parents. Use it when its `files` directory has the same mount ID and
   `st_dev` as the item's parent. Compare with the trash directory, never with `$HOME`.
4. Otherwise use the volume trash under the top directory (item 7), and require the mount
   ID and `st_dev` of its `files/` to equal the parent's. When they differ, refuse and
   name the cause: a different `st_dev` under one mount ID is a **nested btrfs subvolume**;
   a different mount ID is a bind mount or an over-mount.
5. Refuse when the top directory's filesystem type is in GLib's system-internal type list
   (`tmpfs`, `autofs`, `overlay`, `proc`, `sysfs`, ...). **Do not** copy GLib's mount-path
   list or its mount-root rule.
6. Honour `x-gvfs-notrash` for the top directory if `/etc/fstab` carries it (refuse), and
   let `x-gvfs-trash` override item 5. Reading `/run/mount/utab` is not required.
7. `$topdir/.Trash` qualifies only if `lstat` says directory, not symbolic link, sticky bit
   set. Then `$topdir/.Trash/$uid`: create `0700` if absent; require a real directory owned
   by the effective uid. On any failure fall through to `$topdir/.Trash-$uid`: create
   `0700` if absent, then re-`lstat` and require a real directory owned by the effective
   uid; if it was just created and fails the check, remove it.
8. In the chosen trash, `info` and `files` are created `0700` if absent and must be real
   directories, not symbolic links. Open each once with `O_DIRECTORY|O_NOFOLLOW` and do all
   later work relative to those descriptors.
9. **No fallback to the home trash** from a volume. If no volume trash qualifies, refuse.
10. Before trashing a directory the user owns, walk it without following symbolic links
    and refuse if any entry lies inside a subdirectory owned by another uid, naming the
    first such path.
11. Still refused before any of this: what the Guard already refuses (paths inside any
    trash, mount points).

### B. `EXDEV`

12. Items 3–4 make `EXDEV` rare. When `rename` still returns it (overlay lower-layer
    directory, anything untested), remove the info file just written and refuse with
    "cannot be moved to the Trash on `<topdir>` without copying it".
13. **Never copy.** No configuration switch in the first version.
14. Network mounts get no special case: the ownership check in item 7 decides. A refusal
    there reads "the Trash directory on `<topdir>` is not owned by you (uid mapping?)".

### C. Writing the info file

15. Trash name: the item's file name as bytes. On collision insert `.N` before the first
    dot, from 2, as GLib does, so GNOME users see familiar names. A name is taken if
    `info/<name>.trashinfo` **or** `files/<name>` exists.
16. Claim the name with `openat(info_fd, "<name>.trashinfo", O_WRONLY|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC, 0600)`.
    `EEXIST` → next `N`. `ENAMETOOLONG` → drop 10 bytes (then more if needed) from the
    front of the stem on a UTF-8 boundary when the name is valid UTF-8, else on a byte
    boundary; restart at `N = 1`; give up with a refusal when nothing is left.
17. Content, exactly: `[Trash Info]\nPath=<p>\nDeletionDate=<d>\n`.
    `<p>` is the absolute path in the home trash and the path relative to the top directory
    in a volume trash (absolute only if the item is somehow not under it), never containing
    `..`, taken from the canonical parent plus the item's name. Every byte except
    `A–Z a–z 0–9 - . _ ~ /` is written `%XX`, uppercase hex. Non-UTF-8 names need no special
    path. `<d>` is local time, `YYYY-MM-DDThh:mm:ss`, no offset.
18. Write the whole content, `fsync` the file, close it, and only then move the payload
    with `renameat2(..., RENAME_NOREPLACE)`; on `EINVAL` fall back to plain `renameat`
    after re-checking that `files/<name>` does not exist. Do not `fsync` directories.
19. If the move fails for any reason, `unlinkat` the info file and refuse. If that unlink
    fails too, say so: the message names the stray info file.
20. Reading: first line must be `[Trash Info]`; first `Path=` and first `DeletionDate=`
    win; split on the first `=`; decode percent-escapes to bytes leniently (either hex
    case, malformed escapes kept literally); accept `YYYY-MM-DD` or `YYYYMMDD`, ignore
    fractional seconds, `Z` or an offset; an absent or bad date is "unknown". A relative
    `Path` resolves against the trash's top directory (for the home trash, against
    `$XDG_DATA_HOME`); an absolute one is accepted in any trash; a `Path` with a `..`
    component makes the item "cannot be put back". No malformed file may panic or hide
    other items.

### D. `directorysizes`

21. Never read for display. Never add lines.
22. After putting back or emptying a directory item, rewrite the file without that item's
    line through a uniquely named `O_EXCL` temporary file in the same trash directory and
    `rename`; after emptying a whole trash, delete the file. Failures are ignored.
    Unparseable lines are preserved as they are.

### E. Put back

23. Destination is the path from item 20. If anything exists there (`lstat`, so a dangling
    symbolic link counts), refuse: "`<path>` already exists". Never overwrite, never merge.
24. Missing parent directories are **created** (two of three implementations do), mode
    `0777 & ~umask`, and the result says which directories were created.
25. Move with `renameat2(RENAME_NOREPLACE)`, falling back as in item 18. On `EXDEV`
    refuse: "`<path>` is on another filesystem; the item is still in the Trash at
    `<files path>`". No copy.
26. After a successful move remove the info file, then the `directorysizes` line. If the
    info file cannot be removed, the put back still counts as done and the stray record is
    reported.
27. **Payload without info file**: listed, sized, labelled as having no record; it can be
    emptied and cannot be put back.
28. **Info file without payload**: not listed as an item. Counted per trash as stale
    records. Removed during Empty only when the info file is older than a grace period
    (suggest 5 minutes) so that another implementation's in-flight trash is never damaged.
29. **`$trash/expunged/`**: its size counts towards that trash's total, shown as one line
    ("left behind by GNOME"), and Empty removes its children.

### F. Emptying

30. Empty acts on **the items that were listed** plus the orphans of items 27–29, not on
    whatever `files/` contains by the time it runs.
31. Per item: remove the payload; only when it is completely gone (or was already absent)
    remove the info file, then the `directorysizes` line.
32. Removal is descriptor-relative throughout: open directories with
    `openat(..., O_DIRECTORY|O_NOFOLLOW)` (or `openat2` with
    `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS|RESOLVE_NO_XDEV`), `unlinkat` files and symbolic
    links, `unlinkat(AT_REMOVEDIR)` directories after their contents. A symbolic link is
    unlinked, never entered. A mount point inside a payload is not entered and makes that
    item fail.
33. A directory owned by the user that lacks owner `rwx` gets `fchmod` to add it and one
    retry. Nothing else about the payload is changed.
34. `ENOENT` anywhere is success. `EPERM`/`EACCES`/`EROFS`/`EBUSY` leave the item in the
    Trash **with its info file**, and Empty carries on with the rest.
35. The result reports counts and bytes: items removed, items kept, with the first kept
    path and its reason per trash (not owned by you; immutable; read-only volume; mounted
    filesystem inside).
36. A trash on a read-only mount (`ro` in `mountinfo`) is listed, and Empty skips it with
    one message instead of failing per item. A volume that is not mounted has no trash to
    list; no memory of it is kept.

### G. Discovery

37. The home trash, plus, for every distinct mount point in `/proc/self/mountinfo` (last
    record per mount point wins): skip it if its filesystem type is in the item-5 list
    (this is what keeps idle `autofs` points from being triggered); otherwise probe
    `$top/.Trash/$uid` and `$top/.Trash-$uid`, and list both when both exist.
38. Validity when listing is the same as when trashing: `.Trash` must be a real, sticky
    directory; the uid directory a real directory owned by the effective uid; `info` and
    `files` real directories, not symbolic links. An invalid trash is not listed, not
    emptied, and reported once with its reason. World-writable `info` or `files` also
    disqualifies it, as in trash-cli.
39. Probes use `statx` with `AT_SYMLINK_NOFOLLOW|AT_NO_AUTOMOUNT`.
40. Mounts whose type is a network or FUSE filesystem (`nfs`, `nfs4`, `cifs`, `smb3`,
    `9p`, `ceph`, `fuse.*`) are probed and walked on a worker that the screen never joins:
    the list appears with local trashes at once, a network trash shows "checking…" and
    after a deadline (suggest 3 seconds with no progress) "not responding", and the worker
    is abandoned rather than waited for.
41. Discovery is repeated each time the Trash screen is opened, so a volume plugged in
    since the last visit appears.

---

## What stayed INFERRED

- GLib's end-to-end outcome when `$XDG_DATA_HOME` is on another device (code path read,
  not reproduced).
- That "same mount ID and same `st_dev`" predicts `rename()` success on filesystems other
  than the ones tested (btrfs, bind mount, overlay).
- Every network-filesystem behaviour in 2.2 other than the quoted `open(2)` text: uid
  mapping, sticky bit on CIFS/FAT, `EXDEV` inside NFS exports and FUSE unions.
- overlayfs with `redirect_dir=on` (could not be mounted unprivileged) and that the option
  is visible in `mountinfo`.
- Qt's exact percent-encoding set and that `Qt::ISODate` of a local time carries no offset.
- The interoperability consequence of KDE's millisecond `mtime`, and that KDE's orphan
  sweep leaves non-empty directories behind.
- The `fsync` judgement, the refuse-over-copy judgement, the grace period for stale info
  files, and the whole concurrency analysis in section 6: design reasoning, not sourced.
- That reading `/proc/self/mountinfo` cannot block, and that `AT_NO_AUTOMOUNT` /
  `AT_STATX_DONT_SYNC` reduce but do not remove the dead-server hang.

## What this pass did not settle

- Whether refusing on `/` of a subvolume-rooted install (no writable `/.Trash-$uid`) is
  acceptable, or whether an administrator hint ("create a sticky `/.Trash`") belongs in the
  message. The behaviour is correct under the specification; the wording is a product call.
- Whether items spacemap trashes on a btrfs subvolume mount such as `/data` being invisible
  in GNOME Files (GVfs ignores that mount, item 5's deliberate divergence) is acceptable.
  KDE and trash-cli will show them.
- A real NFS or CIFS test of items 7, 14 and 40.
