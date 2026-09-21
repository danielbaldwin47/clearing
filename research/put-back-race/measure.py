"""Measures the Put Back race: trashes batches with a pause between calls and counts how many
items get a Put Back record (`ptbL`) in ~/.Trash/.DS_Store. Prints one JSON document."""

import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time

from ds_store import DSStore

# (pause between calls, seconds the process stays alive after the last call)
CONDITIONS = [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (2.0, 0.0), (0.0, 5.0), (0.5, 5.0)]
REPS = 3
BATCH = 10
SETTLE = 5
HOME = os.path.expanduser("~")
TRASH = os.path.join(HOME, ".Trash")
WORK = os.path.join(HOME, "put-back-race")
TRASHBATCH = sys.argv[1]


def put_back_names():
    """Names in the Trash that carry a Put Back location, or an error string."""
    try:
        with tempfile.TemporaryDirectory() as tmp:
            copy = os.path.join(tmp, "DS_Store")
            shutil.copy(os.path.join(TRASH, ".DS_Store"), copy)
            names = set()
            with DSStore.open(copy, "r") as store:
                for entry in store:
                    code = entry.code.decode() if isinstance(entry.code, bytes) else entry.code
                    if code == "ptbL":
                        names.add(entry.filename)
            return names
    except Exception as error:  # the file is Finder's and may be missing or mid-write
        return f"{type(error).__name__}: {error}"


def make_items(label):
    os.makedirs(WORK, exist_ok=True)
    paths = []
    for i in range(BATCH):
        path = os.path.join(WORK, f"pb-{label}-{i:02d}.txt")
        with open(path, "w") as f:
            f.write(label)
        paths.append(path)
    return paths


def count(trashed):
    names = put_back_names()
    if isinstance(names, str):
        return {"error": names}
    kept = [name in names for name in trashed]
    return {"kept": sum(kept), "of": len(trashed), "which": "".join("1" if k else "0" for k in kept)}


def run_swift(label, pause, linger):
    out = subprocess.run(
        [TRASHBATCH, str(pause), str(linger), *make_items(label)], capture_output=True, text=True, timeout=300
    ).stdout
    return [line.split("\t")[1] for line in out.splitlines() if "\t" in line]


def run_finder(label, one_call):
    paths = make_items(label)
    items = [f'POSIX file "{p}"' for p in paths]
    scripts = [", ".join(items)] if one_call else items
    for script in scripts:
        subprocess.run(
            ["osascript", "-e", f'tell application "Finder" to delete {{{script}}}'],
            capture_output=True, text=True, timeout=120,
        )
    return [os.path.basename(p) for p in paths]


def main():
    result = {
        "macos": platform.mac_ver()[0],
        "arch": platform.machine(),
        "finder_running": subprocess.run(["pgrep", "-x", "Finder"], capture_output=True).returncode == 0,
    }
    try:
        result["trash_readable"] = len(os.listdir(TRASH))
    except Exception as error:
        result["trash_readable"] = f"{type(error).__name__}: {error}"

    runs = []
    for rep in range(REPS):
        order = CONDITIONS[rep:] + CONDITIONS[:rep]
        for pause, linger in order:
            label = f"r{rep}-p{pause}-l{linger}".replace(".", "_")
            trashed = run_swift(label, pause, linger)
            time.sleep(SETTLE)
            runs.append({"method": "trashItem", "pause": pause, "linger": linger, "rep": rep, "trashed": trashed,
                         "after_settle": count(trashed)})
    for one_call in (True, False):
        label = "finder-list" if one_call else "finder-each"
        try:
            trashed = run_finder(label, one_call)
            time.sleep(SETTLE)
            runs.append({"method": label, "pause": 0.0, "rep": 0, "trashed": trashed,
                         "after_settle": count(trashed)})
        except Exception as error:
            runs.append({"method": label, "error": f"{type(error).__name__}: {error}"})

    time.sleep(30)
    for run in runs:
        if "trashed" in run:
            run["at_end"] = count(run.pop("trashed"))
    result["runs"] = runs
    print(json.dumps(result, indent=1))


main()
