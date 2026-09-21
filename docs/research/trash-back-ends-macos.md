# Trash back ends on macOS: what the app does itself

Research notes for the macOS half of issue #21 ("Trash back ends: what spacemap does
itself"). Planning only; no production code changed. Written on Linux with no Mac, from
primary sources: Apple documentation and SDK headers, Apple Developer Forums answers from
Developer Technical Support (DTS), `apple-oss-distributions` source, crate source as
vendored in `~/.cargo/registry`, and this repo's own runner measurements.

It builds on three earlier notes and does not repeat them:
`docs/research/native-trash.md` (branch `research/native-trash`),
`docs/research/put-back-race.md` (branch `research/put-back-race`) and
`docs/research/empty-macos-trash.md` (branch `research/empty-macos-trash`).

**VERIFIED** means the claim was read in the source named beside it, or measured on the
runners in one of the earlier notes. **INFERRED** means a conclusion drawn from those, or
a claim that rests only on third-party reports. Every INFERRED claim is collected again
in "What stayed INFERRED" with the check that would settle it.

**In brief.**

1. Call `trashItemAtURL:resultingItemURL:error:` directly through `objc2` 0.6 and
   `objc2-foundation` 0.3, the two crates the `trash` crate already pulls in. About 20
   lines; it returns the resulting Trash path, which the `trash` crate throws away.
2. Ask Finder with one `osascript` call that first counts the Trash (macOS 26 is reported
   to hang on `empty trash` when the Trash is empty), runs under an app-side deadline,
   and maps the trailing `(-NNNN)` in stderr to a sentence. Sending the Apple Event
   in-process is not worth its cost.
3. Do not probe Automation consent at startup. `AEDeterminePermissionToAutomateTarget`
   has a DTS-confirmed hang, and every GUI-session test gives a false "no" over SSH, where
   the runner showed Finder answering. Decide when the user presses Empty.
4. Put back from the app's own log, persisted across sessions. Do not parse `.DS_Store`:
   records are keyed by the name in the Trash and outlive the item, so a stale record can
   name the wrong original location.
5. List `~/.Trash` plus `<mount>/.Trashes/<uid>` for each local, browsable, writable
   mount from `getfsstat`. Put back with `renamex_np(..., RENAME_EXCL)`.
6. Never reconstruct the original name from the name in the Trash. The log's original
   path is the only source.

---

## 1. Calling `trashItemAtURL:resultingItemURL:error:` without the `trash` crate

### 1.1 What the `trash` crate does on macOS today

Read in `trash` 5.2.9, `src/macos/mod.rs` and `src/lib.rs` — **VERIFIED**:

- `delete_using_file_mgr` builds an `NSString` from the path, calls
  `NSURL::fileURLWithPath`, then
  `file_mgr.trashItemAtURL_resultingItemURL_error(&url, None)`. The `None` discards the
  resulting URL. On error it returns `Error::Unknown` with the text
  ``While deleting '{path:?}', `trashItemAtURL` failed: {err}``; the NSError domain and
  code are flattened into that string.
- Before that, `canonicalize_paths` (`src/lib.rs`) makes a relative path absolute against
  the current directory, canonicalises **the parent only** and re-joins the file name, so
  a symbolic link is trashed as a link rather than followed. It returns
  `Error::TargetedRoot` when the path has no parent.
- A non-UTF-8 path is percent-encoded byte by byte (`percent_encode`) and then passed to
  `fileURLWithPath`. The crate's test for this creates an HFS+ disk image
  (`create_hfs_volume`), because the trick only works there.
- Its macOS dependencies (`Cargo.toml`): `objc2` `0.6.2`, `objc2-foundation` `0.3.2` with
  `default-features = false` and features `std`, `NSError`, `NSFileManager`, `NSString`,
  `NSURL`, plus `percent-encoding` and `log`.
- There is no list, restore or purge on macOS (established earlier).

### 1.2 What the app needs instead

Crates and versions, as already locked in this repo's `Cargo.lock` — **VERIFIED**:
`objc2` 0.6.4, `objc2-foundation` 0.3.2, `objc2-encode` 4.1.0 (pulled by `objc2`),
`bitflags` (pulled by `objc2-foundation`, already used by `ratatui` and `crossterm`).

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-foundation = { version = "0.3", default-features = false, features = [
    "std", "NSError", "NSFileManager", "NSString", "NSURL",
] }
```

Facts the sketch rests on, all read in `objc2-foundation` 0.3.2 source — **VERIFIED**:

- `NSFileManager::trashItemAtURL_resultingItemURL_error(&self, url: &NSURL,
  out_resulting_url: Option<&mut Option<Retained<NSURL>>>) -> Result<(), Retained<NSError>>`
  is a safe function (`src/generated/NSFileManager.rs`), gated on features `NSError` and
  `NSURL`. `NSFileManager::defaultManager()` is safe too.
- `NSURL::from_file_path(path) -> Option<Retained<NSURL>>` (`src/url.rs`, feature `std`)
  passes the path's raw bytes to
  `initFileURLWithFileSystemRepresentation:isDirectory:relativeToURL:`. It returns `None`
  for an empty path or one with an interior NUL. It does not resolve symbolic links.
- `NSURL::to_file_path(&self) -> Option<PathBuf>` uses
  `getFileSystemRepresentation:maxLength:` with a fixed 1024-byte buffer (`PATH_MAX`) and
  returns `None` when that fails. The result is raw bytes, so no UTF-8 round trip.
- `NSError::domain()`, `code()` and `localizedDescription()` exist
  (`src/generated/NSError.rs`); `NSString` implements `Display`.
- `NSFeatureUnsupportedError` is `3328` (`src/generated/FoundationErrors.rs`, feature
  `FoundationErrors`; generated from the SDK header).

Sketch (not compiled: this machine has no Apple target installed):

```rust
#[cfg(target_os = "macos")]
pub struct TrashFailure { pub domain: String, pub code: isize, pub text: String }

#[cfg(target_os = "macos")]
pub fn trash_item(path: &std::path::Path) -> Result<std::path::PathBuf, TrashFailure> {
    use objc2::rc::autoreleasepool;
    use objc2_foundation::{NSFileManager, NSURL};

    // One pool per item: Foundation autoreleases internally, and a worker thread has no pool.
    autoreleasepool(|_| {
        let bad = |text: &str| TrashFailure { domain: String::new(), code: 0, text: text.into() };
        let url = NSURL::from_file_path(path).ok_or_else(|| bad("path cannot be made a file URL"))?;
        let mut resulting = None;
        match NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting))
        {
            Ok(()) => resulting
                .and_then(|url| url.to_file_path())
                .ok_or_else(|| bad("moved to the Trash, but macOS did not say where")),
            Err(error) => Err(TrashFailure {
                domain: error.domain().to_string(),
                code: error.code(),
                text: error.localizedDescription().to_string(),
            }),
        }
    })
}
```

The "moved, but macOS did not say where" arm matters: the move has happened, so the
session log must record the item as trashed with an unknown Trash path (put back
unavailable) rather than report a failure.

### 1.3 Autorelease pool, threads, run loop

- **Pool.** Apple, *Advanced Memory Management Programming Guide*, "Using Autorelease Pool
  Blocks": "Cocoa always expects code to be executed within an autorelease pool block,
  otherwise autoreleased objects do not get released and your application leaks memory";
  "If you spawn a secondary thread. You must create your own autorelease pool block as
  soon as the thread begins executing; otherwise, your application will leak objects";
  and "If you write a loop that creates many temporary objects. You may use an autorelease
  pool block inside the loop". — **VERIFIED.** The `objc2` docs say `autoreleasepool` is
  "mostly useful for preventing leaks (as any Objective-C method may autorelease
  internally)" — **VERIFIED** (docs.rs, `objc2::rc::autoreleasepool`). So: one pool per
  item inside the batch loop. Missing it leaks; it does not crash.
- **Worker thread.** Apple, `FileManager`, "Threading considerations": "The methods of the
  shared FileManager object can be called from multiple threads safely." — **VERIFIED.**
  The delegate caveat in the same paragraph does not apply; the app sets no delegate.
- **No run loop.** The two measurement programs (`trashbatch.swift`, `trashitems.swift`)
  are command-line tools that call `trashItem` from the main thread with no run loop and
  only `Thread.sleep` between calls; every call succeeded and returned its resulting URL on
  macOS 14, 15 and 26 — **VERIFIED** by those runs. That it behaves the same from a
  non-main thread is **INFERRED** from the thread-safety sentence above; nothing measured
  it. The Put Back records still need the process alive about 2 s after the last move
  (established), and that wait needs no run loop either: the 5 s `Thread.sleep` linger
  kept 120 of 120.

### 1.4 Cost against the `trash` crate

From this repo's `Cargo.lock` — **VERIFIED**: `objc2` and `objc2-foundation` have no
dependent other than `trash`; `percent-encoding` has none other than `trash`; `log` is also
used by `mio`. Dropping `trash` therefore removes two crates from the macOS build (`trash`,
`percent-encoding`) and keeps `objc2`, `objc2-encode`, `objc2-foundation`.

**INFERRED, not measured** (no Apple target on this machine, and no runner was launched):
binary size and build time change by a negligible amount in either direction, because the
heavy part, `objc2-foundation` with five features, stays. The reason to drop the crate is
the resulting URL and the structured error, not size.

### 1.5 What else the crate adds, and whether to keep it

| Behaviour | Keep? |
|---|---|
| Parent-only canonicalisation, so a symbolic link is trashed as a link | Keep the property. The app already holds absolute paths and re-verifies identity before the move (`src/trash.rs`). `from_file_path` does not resolve links (VERIFIED, 1.2). That `trashItem` itself does not follow a final symbolic link is **INFERRED**. |
| `TargetedRoot` refusal | Already covered by the Guard. |
| Percent-encoding of non-UTF-8 paths | Do not copy. `objc2-foundation`'s own doc comment on `from_file_path` says it "relies on a quirk of HFS+ that b\"\\xf8\" and b\"%F8\" refer to the same file", that "Modern Apple disk drives use APFS nowadays, which forces all paths to be valid unicode", and that `NSFileManager` APIs "assume that they can always get unicode paths _back_ by calling `NSURL::path` internally, which is not true" — **VERIFIED.** The app refuses non-UTF-8 paths on macOS today; keep that refusal, with the reason "macOS cannot move this name to the Trash". |
| `DeleteMethod::Finder` | Not wanted (established: moves go through `trashItem`). |

---

## 2. Asking Finder to empty the Trash

### 2.1 What Finder's scripting dictionary says

Apple does not publish the dictionary. A copy of `Finder.sdef` kept by the JXA project
(<https://github.com/JXA-userland/JXA/blob/master/packages/@jxa/types/tools/sdefs/Finder.sdef>,
read at commit `5c46bdb`) says — **VERIFIED against that copy, which is third-party**:

- `<command name="empty" code="fndrempt" description="Empty the trash">`, with an optional
  direct parameter ("“empty” and “empty trash” both do the same thing") and an optional
  `security` parameter described as "(obsolete)".
- `<class name="trash-object" code="ctrs" ...>` has exactly one property of its own:
  `warns before emptying` (`warn`, boolean), "Display a dialog when emptying the trash?".
- `<command name="delete" code="coredelo" description="Move an item from its container to
  the trash">`. This closes open point 2 of `native-trash.md` as far as a third-party copy
  can.
- The `item` class has no property for an original location (its properties: name,
  displayed name, name extension, extension hidden, index, container, disk, position,
  desktop position, bounds, label index, locked, kind, description, comment, size, physical
  size, creation date, modification date, icon, url, owner, group, the three privileges,
  information window, properties, class). `original item` exists only on `alias file`.
  Used again in section 4.

### 2.2 The invocation

```
osascript \
  -e 'with timeout of 3600 seconds' \
  -e 'tell application "Finder"' \
  -e 'if (count of items of trash) is 0 then return "already-empty"' \
  -e 'set warned to warns before emptying of trash' \
  -e 'set warns before emptying of trash to false' \
  -e 'try' \
  -e 'empty trash' \
  -e 'on error message number code' \
  -e 'set warns before emptying of trash to warned' \
  -e 'error message number code' \
  -e 'end try' \
  -e 'set warns before emptying of trash to warned' \
  -e 'return "emptied"' \
  -e 'end tell' \
  -e 'end timeout'
```

Why each part:

- **The count guard.** Michael Tsai's blog, "Tahoe AppleScript Timeouts" (2025-09-17),
  relays a reader report: "it is no longer permitted to `tell application "Finder" to empty
  trash` when the trash is already empty: instead of erroring out or, as before, working
  silently, the operation now hangs indefinitely", ending in error -1712 after two minutes;
  the workaround given is `if ((items of trash) as list) is not {} then empty trash`. On
  OS X El Capitan the same call failed instead with `(-128)`
  (`sindresorhus/empty-trash#6`). Both are third-party: **INFERRED**. `count items of
  trash` itself answered on macOS 14, 15 and 26 in the empty-Trash measurement —
  **VERIFIED.**
- **`warns before emptying`.** With the property true, Finder may put up its own "Are you
  sure" dialog, possibly behind the terminal, and the script waits for it. Scripters save
  the property, set it false, empty, and restore (MacScripter, "Empty Trash Question") —
  **INFERRED** (third-party; whether a scripted `empty` honours the property differs
  between reports). The `try` block restores the user's setting when `empty` fails. If the
  app kills `osascript` mid-way the setting can stay false; the app's own dialog has
  already confirmed, so the cost is one lost Finder warning until the user re-ticks it.
- **`with timeout`.** Apple, *AppleScript Language Guide*, "with timeout": "By default,
  when an application fails to respond to a command, AppleScript waits for two minutes
  before reporting an error and halting execution", and on expiry "AppleScript stops
  running the script and returns the error "event timed out". AppleScript does not cancel
  the operation—it merely stops execution of the script." — **VERIFIED.** A large Trash
  can take longer than two minutes, so the default would report a failure while Finder is
  still emptying. `ignoring application responses` avoids the wait but "you forego this
  information" (same guide), that is, every error below; not recommended.

### 2.3 Outcomes to map

`osascript(1)` documents only where errors go: "osascript normally prints script errors to
stderr, so downstream clients only see valid results" — **VERIFIED** (man page dated
April 24, 2014). It documents **no exit status**. That a script error exits with status 1
and that stderr ends with the error number in parentheses, as in
`execution error: Not authorized to send Apple events to Finder. (-1743)`, is **INFERRED**
from third-party transcripts (Scripting OS X; `sindresorhus/empty-trash#6` shows the same
shape in German). Parse the last `(-?\d+)` on stderr; treat any non-zero exit without one
as "Finder could not be asked" and show stderr verbatim in the detail line.

| Case | What comes back | Standing |
|---|---|---|
| Emptied | exit 0, stdout `emptied` | INFERRED (the script is ours; unrun) |
| Trash already empty | exit 0, stdout `already-empty`, because of the guard | INFERRED; without the guard: hang then -1712 on macOS 26, or -128 on older systems (third-party) |
| Consent denied earlier | `-1743`. Apple, macOS 10.14 release notes: "If an event is blocked because the user didn't approve that app, the event will fail with the error code: `-1743`"; `AppleEvents.h`: `errAEEventNotPermitted = -1743` | code VERIFIED; stderr text INFERRED |
| Consent not yet given | macOS shows a consent dialog and `osascript` blocks until it is answered. `AppleEvents.h`: "the user being prompted in a secure fashion the first time an application attempts to send an AppleEvent to another application" — VERIFIED. The dialog names the **terminal emulator**, not the app: "When `osascript` runs from Terminal, the dialog identifies "Terminal"" (Scripting OS X) — INFERRED. "Don't Allow" gives -1743 now and on every later try without a new prompt; `tccutil reset AppleEvents <bundle id>` brings the prompt back (same source) — INFERRED | mixed |
| Prompt left unanswered | blocks until the app's deadline | INFERRED |
| Over SSH, user logged in at the Mac | Finder **answered** on all four runners, with consent pre-granted to `/usr/libexec/sshd-keygen-wrapper` (`empty-macos-trash.md`) — VERIFIED there. On a user's Mac that client has no grant, so expect the prompt on the Mac's screen, unseen by the SSH user, then the deadline; or -1743 | INFERRED |
| No GUI login at all, or Finder quit | `tell application "Finder"` tries to launch it. Apple's AppleScript error table lists `-600` "Application isn't running" and `-609` "Connection is invalid." — VERIFIED as codes. Which one appears, or `-10810` (a Launch Services code absent from that table), is INFERRED from Jamf community reports | INFERRED |
| Locked items | Not found in any primary source. Third-party reports vary between Finder asking about locked items and a timeout | INFERRED; after any outcome, re-list the Trash and report what remains |
| Apple Event timeout | `-1712`, "The Apple event has timed out." (Apple's error table) | VERIFIED as a code |
| User cancelled a Finder dialog | `-128`, "User cancelled." (same table) | VERIFIED as a code |

### 2.4 How long it can block, and the deadline

Three waits stack: the consent prompt (unbounded while unanswered — **INFERRED**), Finder's
own work (proportional to the Trash), and the Apple Event timeout (3600 s as written).
`std::process::Child` has no timed wait, so: spawn `osascript` from a worker thread with
stdin null and stdout/stderr piped, poll `try_wait()` every 100 ms, and keep the UI live
with "Finder is emptying the Trash…". The user can leave the wait with Esc; the app then
kills the child. Per Apple's sentence above, killing the script does not stop Finder, so
the wording after Esc is "Stopped waiting. Finder may still be emptying the Trash.", and
the next listing shows the truth. That killing `osascript` leaves `warns before emptying`
false is the cost named in 2.2.

### 2.5 Sending the event without `osascript`

Possible: `objc2-foundation` 0.3.2 binds
`NSAppleEventDescriptor::appleEventWithEventClass_eventID_targetDescriptor_returnID_transactionID`,
`descriptorWithBundleIdentifier` and
`sendEventWithOptions_timeout_error(send_options, timeout_in_seconds)` (features
`NSAppleEventDescriptor`, `NSDate`, `NSError`), with `NSAppleEventSendOptions::NeverInteract`
— **VERIFIED** in the crate source. It would give a real timeout and no child process.

Not worth it — **INFERRED** judgement: the `empty` event is simple (`fndr`/`empt`), but the
count guard and the `warns before emptying` save and restore each need a hand-built object
specifier (`obj ` records with `form`, `want`, `seld`, `from`), which AppleScript compiles
for free; the consent prompt and its attribution are the same either way; a bug in a
hand-built event targets Finder's Trash; and the osascript route costs one process per
Empty, a once-a-session action. **Expected answer confirmed: no.**

---

## 3. "Can Finder be asked?" without showing a prompt

### 3.1 `AEDeterminePermissionToAutomateTarget(target, class, id, askUserIfNeeded = false)`

`AppleEvents.h` (macOS 11.3 SDK copy) — **VERIFIED**: `API_AVAILABLE( macos(10.14) )`.
"If askUserIfNeeded is false, and this application is not yet permitted to send AppleEvents
to the target, then errAEEventWouldRequireUserConsent will be returned" (`-1744`);
"If the current application is not permitted to send the event, errAEEventNotPermitted will
be returned" (`-1743`); "If the target application is not running, then procNotFound will
be returned" (`-600`); permitted is `noErr`. "Thread safe since version 10.14. Do not call
this function on your main thread because it may take arbitrarily long to return if the
user needs to be prompted for consent."

Against it:

- **It can hang.** Apple Developer Forums thread 666528: the call intermittently never
  returns, stuck in `semaphore_wait_trap`. DTS (Quinn "The Eskimo!"): "OK that's clearly a
  bug, and I recommend that you file it as such", and three months later "AFAICT there's no
  progress to report here )-:". Several feedback numbers, one "100% reproducible" when the
  target has no open windows — **VERIFIED** against the thread. A startup check that can
  hang needs its own thread and deadline, for an answer the app may never use.
- **It answers for "the current application", and the emptying is done by a child
  `osascript`.** Which program macOS charges with the request is not documented for
  command-line tools. The runner's privacy database held separate Automation grants for
  `/bin/bash`, `/usr/bin/osascript` and `/usr/libexec/sshd-keygen-wrapper`
  (`empty-macos-trash.md`) — **VERIFIED** — so the client is not always the terminal
  emulator. That the in-process answer can differ from what `osascript` then gets is
  **INFERRED**.
- **Three of its four answers do not mean "no".** `-1744` means pressing Empty will prompt,
  which is fine. `-600` means Finder is not running *now*.
- Calling it from Rust needs an `extern "C"` block against `CoreServices` plus a
  hand-built `AEAddressDesc` for `com.apple.finder` (`AECreateDesc` with
  `typeApplicationBundleID`), or a further binding crate. **INFERRED** cost: about 40 lines
  of `unsafe`.

### 3.2 GUI-session tests

- `launchctl managername`: "This prints the name of the launchd job manager which manages
  the current launchd context" (`launchctl(1)`) — **VERIFIED.** Apple TN2083, Table 1:
  `Aqua` "Has access to all GUI services", `StandardIO` "Runs only in non-GUI login sessions
  (most notably, SSH login sessions)" — **VERIFIED.**
- `SSH_CONNECTION` in the environment, or `CGSessionCopyCurrentDictionary` returning NULL,
  test the same thing.
- **All three say "no" over SSH, and over SSH Finder answered** on every runner, trashing
  and counting items (`empty-macos-trash.md`) — **VERIFIED.** TN2083's model (a non-GUI
  bootstrap namespace cannot reach per-session GUI services) predates that behaviour. So
  these tests have a measured false negative. They also have false positives: an Aqua
  session with consent denied, or with Finder quit.

### 3.3 Recommendation: decide when the user presses Empty

No startup check for Finder. At startup the app checks only what is free and silent:
whether `~/.Trash` can be listed (`read_dir`; Full Disk Access is never prompted for, per
Apple: "the person using your app must choose to grant access in System Settings" —
VERIFIED in `native-trash.md`; that a denied `read_dir` shows nothing is **INFERRED**).
Empty is always offered. On press, the confirm dialog says what will happen ("Finder will
empty the Trash. macOS may ask whether <terminal> may control Finder.") and the app runs
2.2 under 2.4. The outcome table in 2.3 then picks the next step: success; or fall back to
removing the items itself when listing works; or "This Trash can only be emptied from
Finder".

Reasoning: every silent probe is wrong in a measured or DTS-confirmed way, and a wrong
"no" hides a working feature for good, while a wrong "yes" costs one failed attempt that
the fallback covers. The only thing a startup probe buys is greying out a key, and the
fallback order already makes Empty work in every case where anything can work. The
session's first attempt is remembered: after `-1743` the app goes straight to the fallback
for the rest of the session.

This changes the established plan in one respect: "at startup the app checks whether
Finder can be asked" becomes "at the first Empty". The listing check stays at startup.

---

## 4. Put back for items the app did not trash

### 4.1 Where the original location lives

- **`.DS_Store` records.** Established: `ptbL` and `ptbN` in `~/.Trash/.DS_Store`. The
  container format is reverse-engineered; Wim Lewis's `DSStoreFormat.pod` (Mac::Finder::
  DSStore 1.00, 2013) says "The format is not documented by Apple", credits "Original
  reverse-engineering effort by Mark Mentovai", and describes records as a 4-byte name
  length, the name in UTF-16, a four-character structure id, a four-character data type
  (`ustr`, `blob`, `long`, `bool`, …) and the value, kept in a B-tree ordered "by
  case-insensitive comparison of their filenames, secondarily sorted on the structure ID"
  — **VERIFIED** as that document's content. It predates `ptbL`/`ptbN` and does not list
  them; nor does the Python `ds_store` documentation.
- **The two records.** Both `ustr` (UTF-16 big-endian with a 4-byte character count), keyed
  by the item's **current name in the Trash**; `ptbN` is the original file name, `ptbL` the
  original parent directory, relative to the volume root, stored on the boot volume in the
  firmlink form `System/Volumes/Data/Users/…/`. Source: the doc comments of `trash-core`
  0.3.2 (`core/src/macos.rs`), which says it cross-checked 62 real records against the
  Python library; the Rust `ds_store` crate agrees on `ustr` for both. **INFERRED**:
  third-party, and this repo's own measurement recorded only whether `ptbL` existed, not
  its value.
- **Records outlive items.** Forensic write-ups use these records to recover names of
  items after the Trash was emptied (Ponder The Bits, 2017; `trash-core` README) —
  **INFERRED.** Together with "keyed by the name in the Trash" this is the decisive risk: a
  name reused in the Trash can match a stale record that names another item's home.
- **Records are often missing.** Measured here: a process that exits at once leaves 1 of
  10 items with a record (`put-back-race.md`) — **VERIFIED.** Any other tool that uses
  `trashItem` the way the `trash` crate does produces such items.
- **AppleScript.** Finder's dictionary has no original-location property on items (2.1) —
  **VERIFIED** against the third-party copy.
- **Spotlight and extended attributes.** No primary source describes a `kMDItem*` key or an
  extended attribute carrying the original path of an item in `~/.Trash`; `trash-core`'s
  notes say "no put-back extended attribute". **INFERRED.** Unconfirmed recollection, no
  source found in this pass: the iCloud Drive Trash may use `com.apple.trash.put-back.*`
  attributes. One command on a Mac settles both: `xattr -l ~/.Trash/* ; mdls ~/.Trash/<item>`.

### 4.2 Rust crates that read `.DS_Store`

From the crates.io API and the repositories, 2026-09-21 — **VERIFIED**:

| Crate | Version | Licence | State |
|---|---|---|---|
| `ds_store` (sinistersnare) | 0.3.0, last release 2022-12-23, 6 versions, about 18 000 downloads | MIT by `LICENSE.md` (`license-file`, so crates.io shows "non-standard") | "passively-maintained" badge; depends on `byteorder` and `chrono`; knows `ptbL` and `ptbN`; its record match ends in `other => Err(Error::UnkonwnStructureType(other))`, so **one unknown record id fails the whole parse** |
| `ds_parser` (philocalyst) | 0.4.0, first release 2026-07-11 | BlueOak-1.0.0 | two months old, no stars or issues; reads and writes |
| `trash-core` (SecurityRonin) | 0.3.2, first release 2026-06-19, about 850 downloads | Apache-2.0 | forensic reader; skips records by data type, typed errors, bounds-checked, fuzz target; three months old |

There is no crate named `dsstore`. None is mature. A reader that skips by data type is
about 250 lines; the cost is owning a closed format.

### 4.3 Risks of reading

- **Drift.** The `trash` crate's maintainer refused this route: "it's a closed format and I
  don't see any guarantee that it's a stable format" (`native-trash.md`). The Python parser
  read the file on macOS 14, 15 and 26 in our measurement — **VERIFIED** — so no drift
  across those three; nothing covers the next release.
- **Finder rewrites the file while we read.** DTS suspects `.DS_Store` contention as the
  cause of the Put Back race (thread 773997). The measurement copied the file before
  parsing and tolerated a failed parse. Whether Finder replaces the file atomically or
  rewrites it in place is **INFERRED** unknown; a reader must copy first, bound every
  offset, and treat any error as "no record".
- **Wrong answer, not just no answer.** Stale records (4.1) make a confident wrong original
  path possible. A put back to the wrong folder is worse than a refusal.
- **Same privacy wall.** `.DS_Store` sits inside `~/.Trash`; when the directory cannot be
  listed the file cannot be read.

### 4.4 Recommendation: (a), with the log persisted

**Put back only from the app's own log, persisted across sessions under the app's state
directory. Items with no log entry show "Put back from Finder".** Not (b).

- Finder's Put Back keeps working for foreign items, and the app keeps its moves
  Put-Back-able by staying alive 2 s (established), so nothing is lost: foreign items have
  a working, Apple-owned path, and the app names it.
- A log entry can be validated; a `.DS_Store` record cannot. Each entry holds the original
  path, the resulting Trash path, and the item's device and inode. A same-volume move is a
  rename, which keeps the inode (**INFERRED** for `trashItem`; `lstat` the resulting path
  right after the move and store what is there, which makes it true by construction). An
  entry is offered for put back only while `lstat(trash path)` still matches; anything else
  means Finder or the user moved, restored or erased it, and the entry is dropped.
- Persisting matters because the Trash outlives the session: without it, yesterday's moves
  by this same app read "put back from Finder".

When `~/.Trash` cannot be listed:

- (a) still shows and sizes the app's own entries, because it knows their exact paths;
  whether `lstat` and `rename` on a known path inside an unlistable `~/.Trash` succeed is
  the open question from `empty-macos-trash.md` — **INFERRED** unknown. If they fail, the
  Trash view shows the log as history ("moved to the Trash at 10:41") with put back greyed
  and the Finder sentence.
- (b) gives nothing at all, since `.DS_Store` is behind the same wall.

---

## 5. Listing and sizing

### 5.1 Per-volume layout

**VERIFIED** on four runners (`empty-macos-trash.md`): on a second APFS volume both Finder
and `trashItem` moved items into `/Volumes/EmptyTrashVol/.Trashes/501`; `ls` of `.Trashes`
fails with `Permission denied` (its mode allows search, not read) while `.Trashes/501`
lists and its contents can be removed; Finder's count follows. Home-volume items go to
`~/.Trash`. The image had `Owners: Disabled`; that a volume with ownership enabled behaves
the same is **INFERRED**. No first-party text names the layout (`native-trash.md`).

Never enumerate `.Trashes`; build `<mount>/.Trashes/<getuid()>` and `lstat` it.

### 5.2 Enumerating volumes

`src/platform.rs` already has `mounts()` on macOS: `getfsstat` into caller-owned storage
with `MNT_NOWAIT` (its comment: "getfsstat writes into caller-owned storage, unlike
getmntinfo's shared buffer") — **VERIFIED.** `getfsstat(2)`: with `MNT_NOWAIT` it "will
directly return the information retained in the kernel to avoid delays caused by waiting
for updated information from a file system that is perhaps temporarily unable to respond"
— **VERIFIED.** Extend it to return `f_flags` and `f_fstypename` beside `f_mntonname`
(all three are `libc::statfs` fields on Apple targets — VERIFIED in `libc` 0.2.189).

Flags, from `xnu` `bsd/sys/mount.h` — **VERIFIED**, all present in `libc`:

| Flag | Value | Header comment | Use |
|---|---|---|---|
| `MNT_LOCAL` | `0x00001000` | "filesystem is stored locally" | require; a dead network mount can block `lstat` |
| `MNT_DONTBROWSE` | `0x00100000` | "file system is not appropriate path to user data" | skip |
| `MNT_RDONLY` | `0x00000001` | "read only filesystem" | skip |
| `MNT_ROOTFS` | `0x00004000` | "identifies the root filesystem" | skip; the home Trash covers it |
| `MNT_SNAPSHOT` | `0x40000000` | "The mount is a snapshot" | skip |

**INFERRED:** the system's own helper volumes (`/System/Volumes/Data`, `VM`, `Preboot`,
`Update`) carry `MNT_DONTBROWSE`, so the filter leaves `~/.Trash` plus real user volumes.
If the Data volume were not flagged, `<Data>/.Trashes/<uid>` simply would not exist and the
`lstat` would drop it. Requiring `MNT_LOCAL` means a network volume's Trash is not listed
even when it has one; the app's own log entries for it still work, since they hold exact
paths.

Sizing is a walk of each Trash directory with the scanner the app already has
(`native-trash.md`).

### 5.3 Volumes with no Trash, and iCloud Drive

- Apple, `NSFeatureUnsupportedError`: "The feature isn't supported, because the file system
  lacks the feature, or required libraries are missing, or other similar reasons", with the
  Discussion "For example, some volumes may not support a Trash folder, so these methods
  will report failure by returning `false` or `nil` and an `NSError` with
  `NSFeatureUnsupportedError`." — **VERIFIED.** Value `3328`, domain `NSCocoaErrorDomain`
  (1.2). IINA handles exactly this error from `trashItem` for "SMB" and volumes with "no
  `.Trashes`" by offering "Delete Permanently" (iina/iina PR 6355) — VERIFIED as that
  project's behaviour.
- **Which volumes:** that exFAT and FAT volumes have no Trash is repeated on user forums
  (Lightroom Queen, Adobe) — **INFERRED**; Finder does create `.Trashes` on some of them.
  Match on domain and code, never on file-system type, and never on text.
- **Error text:** not found in any primary source. **INFERRED** recollection: "“name”
  couldn’t be moved to the trash because the volume “volume” doesn’t have one." Show
  `localizedDescription` verbatim whatever it is.
- **iCloud Drive:** an Apple Community thread places its Trash at
  `~/Library/Mobile Documents/com~apple~CloudDocs/.Trash` — **INFERRED.** Whether
  `trashItem` succeeds there and what URL it returns is unmeasured. The design does not
  depend on it: the log records whatever resulting URL comes back, and a Trash the app does
  not list still appears through its own entries.
- DTS thread 798452 (FB19941168): on some APFS USB drives `trashItem` moves the item but
  Finder's Trash icon does not show it until remount — **VERIFIED** as a report; harmless
  to a log-based design.

### 5.4 Moving an item back without overwriting

`rename(2)` on macOS — **VERIFIED**: `renamex_np(from, to, flags)`; `RENAME_EXCL`: "On file
systems that support it (see getattrlist(2) `VOL_CAP_INT_RENAME_EXCL`), it will cause
`EEXIST` to be returned if the destination already exists"; `ENOTSUP`: "flags has a value
that is not supported by the file system." `libc` 0.2.189 binds `renamex_np`,
`renameatx_np` and `RENAME_EXCL = 0x00000004` — **VERIFIED.**

Support by file system, from `apple-oss-distributions`:

- **HFS+:** `hfs/core/hfs_vfsops.c` sets `VOL_CAP_INT_RENAME_EXCL` — **VERIFIED.**
- **APFS:** closed source. Apple introduced the flag with APFS and ships it as the safe-save
  primitive — **INFERRED.**
- **FAT/exFAT:** `msdosfs_vfsops.c` lists its interface capabilities and
  `VOL_CAP_INT_RENAME_EXCL` is not among them — **VERIFIED** for the open-source kext;
  exFAT's driver is closed and newer systems move these file systems to FSKit —
  **INFERRED** same.
- **What the kernel does regardless:** `xnu` `bsd/vfs/vfs_syscalls.c` returns `EEXIST`
  itself when the target exists and `VFS_RENAME_EXCL` is set (unless it is the same file
  differing only in case on a case-insensitive volume), before the file system is asked;
  only when the target is absent does `vn_rename` call `VNOP_RENAMEX`, which an
  unsupporting file system answers with `ENOTSUP` (`bsd/vfs/kpi_vfs.c`) — **VERIFIED.**

So: call `renamex_np(trash_path, original_path, RENAME_EXCL)`. `0` is done. `EEXIST` means
something now occupies the original path: ask, never overwrite. `ENOTSUP` means the target
was absent a moment ago on a file system without the flag: fall back to `lstat` (must be
`ENOENT`) then `rename`, accepting a race window of microseconds on FAT-class volumes.
`ENOENT` means the original parent folder is gone: offer to recreate it (`create_dir_all`)
and retry, since silently recreating folders changes more than was asked. `EXDEV` cannot
happen for a per-volume Trash; if it does (home Trash, item from elsewhere), report it and
do not copy.

---

## 6. Collision renaming, and what put back must reverse

- Apple, `trashItem`: "The actual name of the item may be changed when moving it to the
  trash, so use this URL to access it." (`native-trash.md`) — **VERIFIED.** The deprecated
  `recycleOperation` adds: "If a file with the same name currently exists in the trash
  folder, the new file is renamed." — **VERIFIED.** Neither says how.
- **The pattern** is undocumented. Reports give a time suffix before the extension
  ("name 10.23.45 AM.ext") for Finder, and `trash-core`'s fixture has "report 2.pdf" for
  "report.pdf" — **INFERRED**, and the two disagree, which is the point: the form depends
  on the macOS version and the locale's time format. **Never parse it.** The log stores the
  original path and the resulting path as two independent facts.
- **What put back reverses:** only the rename. Nothing else is known to change: a
  same-volume rename keeps inode, dates, permissions, flags and extended attributes
  (**INFERRED** for `trashItem`; no source says it touches the item).
- **What put back leaves behind:** the item's `ptbL`/`ptbN` records stay in `.DS_Store`.
  Records already outlive emptied items (4.1), and the measurement found "leaving
  `.DS_Store` behind does no harm" — **VERIFIED** for emptying, **INFERRED** for put back.
  Do not edit `.DS_Store`.
- **Edge cases the log must carry:** the original parent may be gone (5.4); the original
  path may be occupied (5.4); the item may have been put back or erased by Finder meanwhile
  (4.4 identity check); a folder trashed with contents is one entry, put back as one
  rename.

---

## What this means for the decision

Written as behaviours an implementer can turn into acceptance criteria.

**1. The move.**
- macOS moves call `NSFileManager.trashItemAtURL:resultingItemURL:error:` through `objc2`
  0.6 and `objc2-foundation` 0.3 (features `std`, `NSError`, `NSFileManager`, `NSString`,
  `NSURL`). The `trash` crate and `percent-encoding` leave `Cargo.toml` and `Cargo.lock`.
- Each move runs inside its own `autoreleasepool`, on the worker thread that runs the
  batch, at full speed; the app stays alive 2 s after the last move (established).
- A successful move yields the resulting Trash path as a `PathBuf`, and the log records
  original path, resulting path, device, inode and time. A success with no resulting URL is
  logged as trashed with put back unavailable.
- A failure carries NSError domain, code and `localizedDescription`. Domain
  `NSCocoaErrorDomain` with code 3328 shows "This volume has no Trash" plus the system text,
  and offers nothing destructive in its place.
- Non-UTF-8 paths stay refused on macOS, with a reason.
- Before merging: build and run the move on the macOS runners the repo already has,
  including one move from a spawned thread; that closes the thread and compile INFERREDs.

**2. Emptying through Finder.**
- One `osascript` call, the script in 2.2: count guard, save/clear/restore of `warns before
  emptying`, `with timeout of 3600 seconds`.
- Runs on a worker thread, stdin null; the UI stays live; Esc kills the child and says
  Finder may still be working.
- Outcome by stdout (`emptied`, `already-empty`) or by the last `(-N)` on stderr: `-1743`
  consent denied; `-1712` timed out; `-128` cancelled in Finder; `-600`, `-609`, `-10810`
  or anything else: Finder could not be asked. Every failure shows stderr verbatim in a
  detail line and moves to the established fallback (remove the items itself when listing
  works, else "This Trash can only be emptied from Finder").
- After any outcome the Trash is listed again and what remains is reported, which covers
  locked items without knowing Finder's behaviour.
- No in-process Apple Event.

**3. Whether Finder can be asked.**
- No startup probe: no `AEDeterminePermissionToAutomateTarget`, no `launchctl`, no
  `SSH_CONNECTION` test. Startup checks only whether `~/.Trash` can be listed.
- Empty is always offered. Its confirm dialog names the consent prompt the user may see and
  which program it will name (the terminal).
- The first `-1743` in a session sends later Empty presses straight to the fallback.

**4. Put back.**
- Put back works for every item with a valid entry in the app's log; the log persists
  across sessions in the state directory.
- An entry is valid while `lstat` of its Trash path matches the recorded device and inode;
  otherwise it is dropped silently.
- Items with no entry are listed and sized and show "Put back from Finder". The app never
  reads or writes `.DS_Store`.
- When `~/.Trash` cannot be listed, the Trash view shows the app's own entries only, says
  why, and offers put back if `lstat` on the known path succeeds.

**5. Listing, sizing, moving back.**
- Trash directories: `~/.Trash`, plus `<mount>/.Trashes/<uid>` for each `getfsstat` entry
  with `MNT_LOCAL` and without `MNT_DONTBROWSE`, `MNT_RDONLY`, `MNT_ROOTFS`, `MNT_SNAPSHOT`,
  kept when `lstat` finds a directory. `.Trashes` itself is never read.
- Sizes come from walking; an unreadable Trash directory is shown as such.
- Put back is `renamex_np(..., RENAME_EXCL)`: `EEXIST` asks and never overwrites; `ENOTSUP`
  falls back to `lstat` plus `rename`; `ENOENT` offers to recreate the parent; `EXDEV` is
  reported and nothing is copied.

**6. Names.**
- The original name comes only from the log. No code parses a Trash name.
- Put back reverses the rename and nothing else, and leaves `.DS_Store` alone.

---

## What stayed INFERRED

Each with the check that settles it. Items 1 to 3 run on the existing macOS runners; the
rest need a person with a Mac for about ten minutes in all.

1. The sketch in 1.2 compiles and `trashItem` works from a non-main thread. *Runner build.*
2. Binary size and build time barely change without the `trash` crate. *Runner build,
   before and after.*
3. `trashItem` trashes a final symbolic link rather than its target; a same-volume move
   keeps the inode. *Runner: `stat` before and after.*
4. `osascript` exits 1 on a script error and ends stderr with `(-N)`; the exact stderr for
   denied consent. *`osascript -e 'error number -1743'; echo $?`, and one denied run.*
5. The consent prompt names the terminal emulator; an unanswered prompt blocks until the
   deadline; over SSH the prompt appears on the Mac's screen.
6. Which code comes back with no GUI login or with Finder quit (-600, -609 or -10810).
7. Finder's scripted `empty` honours `warns before emptying`; what it does with locked
   items; that macOS 26 hangs on an empty Trash without the guard.
8. A denied `read_dir` of `~/.Trash` shows no prompt; `lstat` and `rename` on a known path
   inside an unlistable `~/.Trash` (the open check from `empty-macos-trash.md`).
9. `ptbL`/`ptbN` format details (UTF-16BE, keyed by Trash name, volume-relative, firmlink
   prefix); records outliving items; no extended attribute or Spotlight key holds the
   original path. *`xattr -l ~/.Trash/*; mdls ~/.Trash/<item>`.* Only matters if option (b)
   is ever reopened.
10. Helper system volumes carry `MNT_DONTBROWSE`; a volume with ownership enabled uses the
    same `.Trashes/<uid>`. *`mount` on a Mac.*
11. Which volumes lack a Trash, and the text of error 3328. *One `trashItem` on a FAT
    stick.*
12. iCloud Drive's Trash location and what `trashItem` returns there.
13. APFS and exFAT support for `RENAME_EXCL`. The design tolerates either answer.
14. The collision-rename pattern. The design never depends on it.

---

## Sources

Apple documentation and headers:

- `FileManager`, "Threading considerations" — <https://developer.apple.com/documentation/foundation/filemanager>
- `NSFeatureUnsupportedError` — <https://developer.apple.com/documentation/foundation/nsfeatureunsupportederror-swift.var>
- *Advanced Memory Management Programming Guide*, "Using Autorelease Pool Blocks" — <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/MemoryMgmt/Articles/mmAutoreleasePools.html>
- *AppleScript Language Guide*, "with timeout", "ignoring" — <https://developer.apple.com/library/archive/documentation/AppleScript/Conceptual/AppleScriptLangGuide/reference/ASLR_control_statements.html>
- *AppleScript Language Guide*, error numbers — <https://developer.apple.com/library/archive/documentation/AppleScript/Conceptual/AppleScriptLangGuide/reference/ASLR_error_codes.html>
- `AppleEvents.h`, macOS 11.3 SDK copy (`AEDeterminePermissionToAutomateTarget`, -1743, -1744) — <https://github.com/phracker/MacOSX-SDKs/blob/master/MacOSX11.3.sdk/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/AE.framework/Versions/A/Headers/AppleEvents.h>
- TN2083, "Daemons and Agents" — <https://developer.apple.com/library/archive/technotes/tn2083/_index.html>
- Man pages `osascript(1)`, `launchctl(1)`, `rename(2)`, `getfsstat(2)`, as mirrored from Xcode — <https://keith.github.io/xcode-man-pages/>

Apple Developer Forums (DTS answers):

- Thread 666528, `AEDeterminePermissionToAutomateTarget` hangs — <https://developer.apple.com/forums/thread/666528>
- Thread 798452, `trashItem` and the Trash icon on some USB drives — <https://developer.apple.com/forums/thread/798452>
- Thread 773997, the Put Back race (from `native-trash.md`) — <https://developer.apple.com/forums/thread/773997>

`apple-oss-distributions`:

- `xnu` `bsd/sys/mount.h`, `bsd/sys/attr.h`, `bsd/vfs/vfs_syscalls.c`, `bsd/vfs/kpi_vfs.c` — <https://github.com/apple-oss-distributions/xnu>
- `hfs` `core/hfs_vfsops.c` — <https://github.com/apple-oss-distributions/hfs>
- `msdosfs` `msdosfs.kextproj/msdosfs.kmodproj/msdosfs_vfsops.c` — <https://github.com/apple-oss-distributions/msdosfs>

Crate source (read in `~/.cargo/registry`, versions from this repo's `Cargo.lock`):

- `trash` 5.2.9: `src/macos/mod.rs`, `src/macos/tests.rs`, `src/lib.rs`, `Cargo.toml`
- `objc2-foundation` 0.3.2: `src/url.rs`, `src/generated/NSFileManager.rs`, `NSError.rs`, `NSAppleEventDescriptor.rs`, `FoundationErrors.rs`, `Cargo.toml`
- `objc2` 0.6.4, `autoreleasepool` — <https://docs.rs/objc2/0.6.4/objc2/rc/fn.autoreleasepool.html>
- `libc` 0.2.189: `src/unix/bsd/apple/mod.rs`
- `ds_store` 0.3.0 — <https://github.com/sinistersnare/ds_store>; `ds_parser` 0.4.0 — <https://github.com/philocalyst/ds>; `trash-core` 0.3.2 — <https://github.com/SecurityRonin/trash-forensic>; crates.io API for versions, dates and licences

This repo:

- `docs/research/native-trash.md`, `docs/research/put-back-race.md`, `docs/research/empty-macos-trash.md` and their scripts (`trashbatch.swift`, `trashitems.swift`, `measure.py`)
- `src/platform.rs` (`mounts()`), `src/trash.rs`, `Cargo.toml`, `Cargo.lock`

Third-party, used only for INFERRED claims:

- Finder scripting dictionary copy — <https://github.com/JXA-userland/JXA/blob/master/packages/@jxa/types/tools/sdefs/Finder.sdef>
- Wim Lewis, `DSStoreFormat.pod` — <https://metacpan.org/dist/Mac-Finder-DSStore/view/DSStoreFormat.pod>; Python `ds_store` docs — <https://ds-store.readthedocs.io/en/latest/>
- Michael Tsai, "Tahoe AppleScript Timeouts" — <https://mjtsai.com/blog/2025/09/17/tahoe-applescript-timeouts/>
- `sindresorhus/empty-trash#6` — <https://github.com/sindresorhus/empty-trash/issues/6>; MacScripter, "Issue With Empty Trash" — <https://www.macscripter.net/t/issue-with-empty-trash/77182> and "Empty Trash Question" — <https://www.macscripter.net/t/empty-trash-question/77264>
- Scripting OS X, "Avoiding AppleScript Security and Privacy Requests" — <https://scriptingosx.com/2020/09/avoiding-applescript-security-and-privacy-requests/>
- IINA PR 6355 — <https://github.com/iina/iina/pull/6355>
- Ponder The Bits, Trash `.DS_Store` forensics — <https://ponderthebits.com/2017/01/mac-dumpster-diving-identifying-deleted-file-references-in-the-trash-ds_store-files-part-1/>
- Apple Community thread on the iCloud Drive Trash — <https://discussions.apple.com/thread/250403637>
