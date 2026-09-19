#!/usr/bin/env python3
"""Create owned, reproducible allocated trees for visual and scan comparisons.

Never overwrites existing files. UI sizes use allocated extents, not sparse holes.
The benchmark has 160,000 regular files across broad and nested directories.
"""
import argparse
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def allocated(path, size):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        if path.stat().st_size != size:
            raise RuntimeError(f"Existing fixture file has unexpected size: {path}")
        return
    with path.open("xb") as f:
        os.posix_fallocate(f.fileno(), 0, size)
        # Written data ensures allocation is real even on copying tools that skip
        # all-zero extents. Benchmark files themselves are fully written below.
        f.write(b"tui-disk deterministic fixture\n")


def story():
    root = ROOT / "fixtures/ssd-story"
    mib = 1024 * 1024
    files = {
        "Caches/Browser/Code Cache/index-archive.bin": 420,
        "Caches/Browser/Media Cache/video-buffer.bin": 610,
        "Caches/Package cache/linux-headers-old.pkg.tar.zst": 260,
        "Caches/Package cache/toolchain-old.pkg.tar.zst": 330,
        "Caches/Thumbnails/thumbnails.db": 180,
        "Games/Orbit/install.pak": 680,
        "Games/Orbit/textures.pak": 260,
        "Games/Signal/save-backup.zip": 220,
        "Downloads/archlinux-old.iso": 410,
        "Downloads/video-export-final.mp4": 240,
        "Downloads/design-assets.zip": 110,
        "Projects/terrain/target/debug/build.bin": 150,
        "Projects/terrain/assets/terrain.raw": 70,
        "Projects/site/node_modules/bundle.cache": 100,
        "Photos/2025/holiday.raw": 130,
        "Photos/2026/screenshot-collection.zip": 80,
        "Documents/research.pdf": 15,
        "Documents/notes.txt": 1,
    }
    for name, size in files.items():
        allocated(root / name, size * mib)
    (ROOT / "fixtures/story-manifest.json").write_text(json.dumps({
        "root": str(root), "files": len(files),
        "apparent_bytes": sum(files.values()) * mib,
        "layout": files, "layout_units": "MiB", "sparse": False,
    }, indent=2) + "\n")
    print(root)


def benchmark(files=160000):
    root = ROOT / "fixtures/benchmark-tree"
    root.mkdir(parents=True, exist_ok=True)
    total = 0
    # Each file has genuinely written data, a distinct name, and an independent
    # inode. One fifth are in deeper project-like paths; all sizes are known.
    block = bytes(range(256)) * 16
    for i in range(files):
        branch = root / f"workspace-{i % 64:02d}"
        if i % 5 == 0:
            branch /= f"project-{(i // 64) % 10:02d}/src/modules/component-{(i // 640) % 10:02d}/cache"
        else:
            branch /= f"package-{(i // 64) % 64:02d}"
        path = branch / f"entry-{i:07d}.dat"
        size = (1 + i % 3) * 4096
        total += size
        if path.exists():
            if path.stat().st_size != size:
                raise RuntimeError(f"Existing benchmark file has unexpected size: {path}")
            continue
        branch.mkdir(parents=True, exist_ok=True)
        with path.open("xb") as f:
            f.write(block * (1 + i % 3))
    os.sync()
    manifest = {"root": str(root), "regular_files": files,
                "apparent_file_bytes": total, "sparse": False,
                "hard_links": False, "symlinks": False,
                "pattern": "64 top-level branches; 64 broad packages and 5-level subtrees"}
    (ROOT / "fixtures/benchmark-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(root)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=["story", "benchmark", "all"])
    parser.add_argument("--files", type=int, default=160000)
    args = parser.parse_args()
    if args.kind in ("story", "all"):
        story()
    if args.kind in ("benchmark", "all"):
        benchmark(args.files)
