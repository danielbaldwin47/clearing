#!/usr/bin/env python3
"""Independent wall-clock scanner comparison in a cold-cache-capable Linux VM.

The harness author is separate from the scanner builder. Each app scans the same
allocated ext4 tree. Guest cache dropping is real; cache=none bypasses host page
cache. Host warm measurements are supplementary, never relabeled as cold.
"""
import argparse
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / ".benchmark-tools"
DEADLINE = datetime.datetime(2026, 9, 20, 0, 46, 22, tzinfo=datetime.timezone.utc).timestamp()


def command(args, **kwargs):
    return subprocess.run(list(map(str, args)), check=True, **kwargs)


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def install_binary(binary, destination, initroot):
    target = initroot / destination.lstrip("/")
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(binary, target)
    dependencies = command(["ldd", binary], capture_output=True, text=True).stdout
    for library in re.findall(r"(?:=>\s+)?(/[^\s()]+)", dependencies):
        source = Path(library)
        dest = initroot / library.lstrip("/")
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, dest)


def prepare(args):
    WORK.mkdir(exist_ok=True)
    spec = {"broad": ("benchmark-tree", "benchmark-manifest.json", "benchmark", 3, 200000),
            "ui": ("ssd-story", "story-manifest.json", "story", 6, 4096)}[args.fixture]
    dirname, manifest_name, create_kind, disk_gib, inode_count = spec
    fixture = ROOT / "fixtures" / dirname
    manifest_path = ROOT / "fixtures" / manifest_name
    if not manifest_path.exists():
        raise SystemExit(f"Create fixture first: python scripts/create_fixture.py {create_kind}")
    manifest = json.loads(manifest_path.read_text())
    disk = WORK / ("benchmark.ext4" if args.fixture == "broad" else "ui-story.ext4")
    if not disk.exists():
        with disk.open("xb") as f:
            f.truncate(disk_gib * 1024**3)
        command(["mkfs.ext4", "-q", "-F", "-b", "4096", "-m", "0", "-N", str(inode_count), "-d", fixture, disk])
        command(["debugfs", "-w", "-R", "rmdir lost+found", disk], capture_output=True)
        if args.fixture == "ui":
            # mke2fs may turn the source's unwritten preallocated extents into
            # holes. Restore full allocation for exactly the existing logical
            # range of each UI file; source names, sizes, and content stay fixed.
            allocations = []
            for relative, mib in manifest["layout"].items():
                size = mib * 1024 * 1024
                source = fixture / relative
                if source.stat().st_size != size or source.stat().st_blocks * 512 < size:
                    raise RuntimeError(f"UI fixture no longer matches allocated manifest: {source}")
                if any(char in relative for char in ('"', '\n', '\r', '\\')):
                    raise RuntimeError("Unsupported character in owned fixture name")
                allocations.append(f'fallocate "/{relative}" 0 {(size + 4095) // 4096 - 1}')
            allocation_script = WORK / "ui-allocations.debugfs"
            allocation_script.write_text("\n".join(allocations) + "\n")
            command(["debugfs", "-w", "-f", allocation_script, disk], capture_output=True)
    initroot = WORK / "initroot"
    # All paths in this staging tree were created by this script.
    if initroot.exists():
        shutil.rmtree(initroot)
    initroot.mkdir()
    for directory in ("bin", "dev", "proc", "sys", "tmp", "tree"):
        (initroot / directory).mkdir()
    command(["gcc", "-static", "-O2", f"-DTRIALS={args.trials}",
             f"-DGDU_FULL_TREE={int(args.gdu_engine == 'full')}",
             ROOT / "scripts/benchmark_guest.c", "-o", initroot / "init"])
    install_binary(args.binary, "/bin/tui-disk", initroot)
    install_binary(shutil.which("gdu"), "/bin/gdu", initroot)
    # cpio receives deterministic sorted names, all owned by root in the guest.
    names = ["."] + sorted(str(p.relative_to(initroot)) for p in initroot.rglob("*"))
    archive = WORK / "initramfs.cpio"
    with archive.open("wb") as output:
        command(["cpio", "--create", "--format=newc", "--owner=0:0", "--quiet"],
                cwd=initroot, input=("\n".join(names) + "\n").encode(), stdout=output)
    return disk, archive, manifest


def parse_output(raw):
    audit_match = re.search(r"BENCH_AUDIT (\{[^\r\n]+\})", raw)
    if not audit_match:
        raise RuntimeError("Missing independent file/directory allocation audit")
    audit = json.loads(audit_match.group(1))
    pattern = r"BENCH_RESULT (gdu|tui-disk) (cold|warm) (\d+) (\d+)\r?\nBENCH_OUTPUT_BEGIN\r?\n(.*?)\r?\nBENCH_OUTPUT_END"
    measurements = []
    for app, cache, trial, nanos, output in re.findall(pattern, raw, re.S):
        output = output.strip()
        if app == "tui-disk":
            summary = json.loads(output)
            total = summary["bytes"]
            if summary["errors"]:
                raise RuntimeError(f"tui-disk reported scan errors: {summary}")
            expected = audit["file_bytes"] + audit["directory_bytes"]
            if summary["files"] != audit["files"] or summary["directories"] != audit["directories"]:
                raise RuntimeError("tui-disk file/directory counts disagree with independent audit")
        else:
            if output.startswith("["):
                gdu_root = json.loads(output)[3][0]
                total = gdu_root["dsize"]
                summary = {"bytes": total, "apparent_bytes": gdu_root["asize"], "items": gdu_root["items"]}
                if summary["items"] != audit["files"] + audit["directories"]:
                    raise RuntimeError("gdu full-tree entry count disagrees with independent audit")
            else:
                total = int(output.split()[0])
                summary = {"bytes": total}
            expected = audit["file_bytes"]
        if total != expected:
            raise RuntimeError(f"{app} allocated bytes {total} disagree with independent expected {expected}")
        measurements.append({"app": app, "cache": cache, "trial": int(trial),
                             "seconds": int(nanos) / 1e9, "summary": summary})
    if "BENCH_COMPLETE" not in raw or "BENCH_ERROR" in raw:
        raise RuntimeError("VM benchmark did not complete cleanly; inspect raw log")
    return measurements, audit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/tui-disk")
    parser.add_argument("--trials", type=int, default=7)
    parser.add_argument("--cpus", type=int, default=20)
    parser.add_argument("--fixture", choices=("broad", "ui"), default="broad",
                        help="broad: 160k-file scan proof; ui: exact ssd-story capture tree")
    parser.add_argument("--gdu-engine", choices=("full", "summary"), default="full",
                        help="full: interactive tree engine with root-only JSON; summary: original faster totals-only baseline")
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--output", type=Path, default=ROOT / "benchmarks/latest.json")
    args = parser.parse_args()
    if time.time() >= DEADLINE:
        raise SystemExit("User's one-hour hard freeze deadline has passed; benchmark will not start.")
    if args.trials < 3:
        raise SystemExit("At least 3 independent trials are required.")
    args.binary = args.binary.resolve()
    WORK.mkdir(exist_ok=True)
    lock = (WORK / "benchmark.lock").open("w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        raise SystemExit("Another benchmark owns the staging tree; wait for it to finish.")
    disk, archive, fixture_manifest = prepare(args)
    # Finish host image writes before starting the guest. Guest sync cannot
    # flush unrelated host writeback; direct I/O alone is not that guarantee.
    with disk.open("rb") as image_file:
        os.fsync(image_file.fileno())
    if args.prepare_only:
        print(f"Prepared {disk} and {archive}")
        return
    # Freeze provenance BEFORE launch; builder may replace the release binary
    # during a run. The VM reads only this staged image, never the live source.
    measured_hashes = {app: sha256(WORK / "initroot/bin" / app) for app in ("tui-disk", "gdu")}
    archive_hash = sha256(archive)
    evidence = WORK / "evidence"
    evidence.mkdir(exist_ok=True)
    snapshot = evidence / f"tui-disk-{measured_hashes['tui-disk']}"
    if not snapshot.exists():
        shutil.copy2(WORK / "initroot/bin/tui-disk", snapshot)
    qemu = WORK / "qemu/usr/bin/qemu-system-x86_64"
    kernel = Path("/usr/lib/modules/7.2.3-arch1-3/vmlinuz")
    if not qemu.exists() or not kernel.exists():
        raise SystemExit("Local QEMU or configured kernel is missing. See benchmarks/README.md.")
    environment = os.environ.copy()
    environment["LD_LIBRARY_PATH"] = str(WORK / "qemu/usr/lib")
    argv = [qemu, "-enable-kvm", "-cpu", "host", "-smp", str(args.cpus), "-m", "4096",
            "-display", "none", "-serial", "stdio", "-monitor", "none", "-no-reboot",
            "-nodefaults", "-nic", "none", "-L", WORK / "qemu/usr/share/qemu",
            "-kernel", kernel, "-initrd", archive,
            "-append", "console=ttyS0 quiet loglevel=0 panic=-1 rdinit=/init",
            "-drive", f"file={disk},format=raw,if=virtio,cache=none,aio=threads,readonly=on"]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    log = args.output.with_suffix(".log")
    started = time.time()
    with log.open("w") as output:
        command(argv, env=environment, stdout=output, stderr=subprocess.STDOUT,
                timeout=min(1800, max(1, DEADLINE - time.time())))
    measurements, audit = parse_output(log.read_text())
    expected_files = fixture_manifest.get("regular_files", fixture_manifest.get("files"))
    if audit["files"] != expected_files:
        raise RuntimeError(f"Guest fixture has {audit['files']} files, expected {expected_files}")
    if sha256(archive) != archive_hash:
        raise RuntimeError("Measured initramfs changed during the VM run; result invalidated")
    if len(measurements) != args.trials * 4:
        raise RuntimeError(f"Expected {args.trials * 4} measurements, got {len(measurements)}")
    summary = {}
    for cache in ("cold", "warm"):
        values = {app: [m["seconds"] for m in measurements if m["app"] == app and m["cache"] == cache]
                  for app in ("gdu", "tui-disk")}
        medians = {app: statistics.median(v) for app, v in values.items()}
        paired_wins = sum(a < b for a, b in zip(values["tui-disk"], values["gdu"]))
        summary[cache] = {"median_seconds": medians, "speedup": medians["gdu"] / medians["tui-disk"],
                          "paired_wins": paired_wins, "trials": args.trials,
                          "won": medians["tui-disk"] < medians["gdu"] and paired_wins > args.trials / 2}
    result = {"schema": 1, "author_role": "independent benchmark author", "unix_started": started,
              "environment": {"kind": "isolated KVM Linux guest", "cpus": args.cpus,
                              "ram_mib": 4096, "filesystem": "ext4", "host_filesystem": "btrfs",
                              "cold": "guest sync(); write 3 to /proc/sys/vm/drop_caches before each scan",
                              "host_image_cache": "QEMU cache=none (O_DIRECT)",
                              "warm": "immediately follows untimed scan by same executable",
                              "scope": "directory-tree metadata and allocator usage; file data is not read by either scanner"},
              "binaries": {"tui-disk": {"path": str(args.binary), "sha256": measured_hashes["tui-disk"],
                                         "measured_snapshot": str(snapshot),
                                         "current_binary_matches_measured": sha256(args.binary) == measured_hashes["tui-disk"]},
                           "gdu": {"path": shutil.which("gdu"), "sha256": measured_hashes["gdu"]}},
              "measured_initramfs_sha256": archive_hash,
              "freeze_deadline_utc": "2026-09-20T00:46:22Z",
              "fixture_kind": args.fixture,
              "gdu_engine": args.gdu_engine,
              "gdu_command": ["gdu", "--config-file", "/dev/null", "-n", "-p", "-c", "--no-prefix", "-s"] +
                             (["-o", "-", "--output-attrs", "asize,dsize,items"] if args.gdu_engine == "full" else []) + ["/tree"],
              "fixture": fixture_manifest,
              "fixture_image": str(disk),
              "independent_allocation_audit": audit,
              "allocation_semantics": {"gdu": "regular-file allocated blocks only",
                                       "tui-disk": "regular-file plus directory allocated blocks"},
              "gate": "lower median and strict majority of paired wins in both cache states",
              "summary": summary, "won": all(s["won"] for s in summary.values()),
              "measurements": measurements, "raw_log": str(log)}
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"won": result["won"], "summary": summary, "output": str(args.output)}, indent=2))


if __name__ == "__main__":
    main()
