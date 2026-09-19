# Independent scanner benchmark

The benchmark harness and fixture were authored by the benchmark agent, which did
not write the scanner and does not judge visual pairs. The scanner builder may
inspect results and optimize application code, but does not edit this harness.

## Fixed comparison protocol

- The main deterministic tree: 160,000 independent, fully written regular files,
  4–12 KiB each, across 64 top-level branches and broad/deep subtrees.
- Both binaries traverse the identical tree on the identical mounted ext4 image.
- Both traverse every entry, retain their full interactive tree, and output only
  a small total summary. The primary gdu command uses its normal parallel engine:
  `gdu --config-file /dev/null -n -p -c --no-prefix -s -o - --output-attrs asize,dsize,items /tree`.
  Export mode selects the same complete analyzer as the TUI; `-s` emits only the
  root aggregate, avoiding recursive export overhead. Verified against installed
  5.37.0 and its version-pinned `report/export.go` source before measuring.
- `tui-disk --scan /tree --summary` uses the release binary and the same scanner
  as the application. It omits recursive JSON serialization, matching gdu's
  small summary output.
- Each measured cold scan follows `sync()` and a successful write of `3` to
  `/proc/sys/vm/drop_caches` inside the isolated guest. This evicts guest page,
  dentry, and inode caches. No host cache setting is changed.
- QEMU `cache=none` uses direct I/O for the ext4 image, bypassing the host image
  page cache. Storage-controller/SSD hardware caches are outside this claim.
- The host image is explicitly `fsync`ed before each VM launch so fixture
  construction writes are complete before timing starts.
- Each measured warm scan immediately follows an untimed full scan by the same
  executable. The order alternates every trial and differs between cache states.
- The monotonic wall-clock timer surrounds process launch through process exit;
  output goes to the same guest temporary filesystem for both applications.
- Seven trials per program/cache state are the acceptance run. A three-trial run
  is only a smoke check. The fixed gate requires a lower median and a strict
  majority of paired wins in **both** cold and warm cache states.
- Both totals must match a separate, untimed POSIX metadata walk. gdu omits
  directory blocks; tui-disk includes them. The audit records each category and
  verifies each tool's declared semantics, plus tui-disk's entry counts.
  Any scanner error, failed
  cache reset, unsuccessful process exit, or incomplete trial invalidates a run.

## Run

```sh
python scripts/create_fixture.py benchmark
cargo build --release
python scripts/benchmark.py --output benchmarks/latest.json
```

The optional second run uses the exact relative names, file sizes, contents,
and hierarchy of the `ssd-story` directory used for application and diskonaut
captures. Its separate 6 GiB ext4 image preserves fully allocated file extents:

```sh
python scripts/benchmark.py --fixture ui --output benchmarks/ui-final.json
```

The original faster totals-only engine remains available as a separate
comparator, with its original results (including the valid cold loss) preserved:

```sh
python scripts/benchmark.py --gdu-engine summary --output benchmarks/summary-latest.json
```

Its command omits `-o - --output-attrs asize,dsize,items`. This engine stores only
top-level totals and was observed to intermittently omit subtrees. The primary
engine changed for complete-tree workload parity and correct completion, not by
changing the speed gate. `--depth=0` is explicitly ignored, so it cannot select
the full-tree analyzer with root-only output. See `gdu-incomplete-summary.md`.

Both fixture modes use the same seven-trial cold/warm protocol and gate. The
small UI fixture is an additional same-tree check; it does not replace the
160,000-file scan-performance proof. Directory allocation can differ between
host Btrfs and guest ext4, and each guest run independently audits those bytes.

`latest.json` contains every timing, binary SHA-256 hashes, summary statistics,
fixture identity, cache definitions, and the gate result. The adjacent `.log`
preserves guest output. The fixture is created once and never tuned against
performance results.

Hashes are captured from the staged measured binaries before the VM starts.
An immutable measured binary copy is retained under `.benchmark-tools/evidence`.
The result explicitly reports whether the current release still matches the
measured copy; rebuilding afterward does not transfer the earlier performance
win. A file lock prevents concurrent runs from replacing the VM staging tree.
The current user deadline is **2026-09-20 00:46:22 UTC** (two hours from the first
command); no benchmark starts afterward, and a running VM is bounded by it.

## Local headless VM dependency

This machine permits unprivileged KVM via `/dev/kvm`, but not host root cache
dropping. QEMU 11.1.1 and its missing runtime dependencies were downloaded from
the official Arch mirror into `.benchmark-tools/downloads` and extracted into
`.benchmark-tools/qemu`; no host packages were installed. The readable installed
Arch kernel `/usr/lib/modules/7.2.3-arch1-3/vmlinuz` includes ext4 and virtio block
drivers. The harness constructs its own small initramfs, with a static C init
and the unchanged scanner binaries plus their dynamic libraries. It opens no
display or network interface.

The VM comparison is a Linux/ext4/KVM result on this machine. It is not a claim
that host Btrfs cache behavior, every filesystem, or every possible tree has been
benchmarked. Both tested applications experience the same VM and filesystem.

## UI fixture

`python scripts/create_fixture.py story` creates `fixtures/ssd-story` with 18
allocated files totaling 4,473,225,216 bytes on the current host. It is separate
from the broad timing fixture and intended for identical app/diskonaut captures.
The fixture manifest records all names and sizes. Do not confirm deletion in
the canonical fixture; use a copy for destructive interaction tests.
