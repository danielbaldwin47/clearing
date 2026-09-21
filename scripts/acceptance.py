#!/usr/bin/env python3
"""Independent black-box checks; all destructive exercises use temporary trees."""
import argparse
import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT / 'target/release/clearing'
ANSI = re.compile(rb'\x1b\[[0-9;?]*[A-Za-z]')
# What a progress screen draws when no cell changed, every 40 ms while busy.
EMPTY_FRAME = b'\x1b[39m\x1b[49m\x1b[59m\x1b[0m\x1b[?25l'
# Seconds without a changed frame that end a read: the app answers a key in 1 ms.
SETTLE = 0.05
# For a send whose case then asserts that nothing happened: no text ends the
# wait, so a wrongful move or delete gets the whole duration to land.
NEVER = b'\0'
# A space map heading. A scan's own screen blanks it, so the app draws it again
# when the scan ends, whatever message the footer then carries. A dialog opening
# dims and so redraws it too: the key that opens one goes in a send of its own.
IDLE = b'LARGEST FIRST'


def scan(path):
    result = subprocess.run([str(BINARY), '--scan', '--json', str(path)], text=True, capture_output=True, timeout=30)
    if result.returncode:
        raise AssertionError(result.stderr)
    return json.loads(result.stdout)


def expected(path):
    """Independent lstat walk: never follows links; hard links consume blocks once."""
    seen = set()
    result = dict(bytes=0, apparent_bytes=0, files=0, directories=0, errors=0)
    stack = [Path(path)]
    while stack:
        entry = stack.pop()
        st = entry.lstat()
        directory = entry.is_dir() and not entry.is_symlink()
        result['directories' if directory else 'files'] += 1
        identity = (st.st_dev, st.st_ino)
        if directory or identity not in seen:
            result['bytes'] += st.st_blocks * 512
            result['apparent_bytes'] += st.st_size
            seen.add(identity)
        if directory:
            try:
                stack.extend(entry.iterdir())
            except PermissionError:
                result['errors'] += 1
    return result


class Session:
    def __init__(self, path, cols=100, rows=34, until=IDLE):
        self.pid, self.master = pty.fork()
        if not self.pid:
            os.environ.update(TERM='xterm-256color', COLORTERM='truecolor')
            os.execv(str(BINARY), [str(BINARY), str(path)])
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
        self.output = bytearray()
        self.closed = False
        self.read(1, until=until)

    def read(self, duration=0.25, until=None):
        """Read until the screen settles, or until `until` shows in the new text.

        `duration` is the ceiling either way, so text that never shows costs the
        whole wait and fails nothing. Work the app does off the key (a scan, a
        `gio` move) draws empty frames while it runs, so it needs `until`.
        """
        start = len(self.output)
        ceiling = time.monotonic() + duration
        settled = None
        visible = 0
        while True:
            deadline = ceiling if until or settled is None else min(ceiling, settled)
            wait = deadline - time.monotonic()
            if wait <= 0 or not select.select([self.master], [], [], wait)[0]:
                break
            try:
                chunk = os.read(self.master, 65536)
            except OSError as e:
                if e.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            self.output.extend(chunk)
            new = bytes(self.output[start:])
            if until:
                if until in ANSI.sub(b'', new):
                    break
            elif (drawn := len(new.replace(EMPTY_FRAME, b''))) != visible:
                visible = drawn
                settled = time.monotonic() + SETTLE
        return bytes(self.output)

    def send(self, data, pause=0.25, until=None):
        os.write(self.master, data)
        return self.read(pause, until)

    def close(self):
        if self.closed:
            return
        self.closed = True
        try:
            os.write(self.master, b'\x03')
            for stop in (None, signal.SIGTERM, signal.SIGKILL):
                if stop:
                    os.kill(self.pid, stop)
                code = self.reap(0.7 if stop is None else 5)
                if code is not None:
                    return code
        finally:
            os.close(self.master)

    def reap(self, seconds):
        """The app's exit code, or None if it outlives `seconds`.

        Reads throughout: a read that ended on its text leaves the rest of the
        frame in the PTY, and macOS holds an exiting process until that is read.
        """
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            pid, status = os.waitpid(self.pid, os.WNOHANG)
            if pid:
                return os.waitstatus_to_exitcode(status)
            self.read(0.05)
            time.sleep(0.01)
        return None


class CommandLineAcceptance(unittest.TestCase):
    def test_version_flags(self):
        package = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']
        for flag in ['--version', '-V']:
            with self.subTest(flag=flag):
                result = subprocess.run([str(BINARY), flag], text=True, capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, f"clearing {package['version']}\n")
                self.assertEqual(result.stderr, '')

    def test_help_lists_version(self):
        result = subprocess.run([str(BINARY), '--help'], text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('-V, --version', result.stdout)
        self.assertEqual(result.stderr, '')

    def test_no_mouse_is_an_unknown_option(self):
        result = subprocess.run([str(BINARY), '--no-mouse', '--scan', '.'], text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stderr, 'clearing: unknown option: --no-mouse\n')
        self.assertEqual(result.stdout, '')

    def test_help_explains_escape_by_scan_context(self):
        result = subprocess.run([str(BINARY), '--help'], text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Esc cancels a rescan or dialog; quits otherwise', result.stdout)
        self.assertNotIn('Esc cancels a scan or dialog', result.stdout)

    def test_help_lists_navigation_and_trash_once(self):
        result = subprocess.run([str(BINARY), '--help'], text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        keys = result.stdout.split('Keys:', 1)[1].split('\n\n', 1)[0]
        for key in ['Home', 'End', 'PgUp', 'PgDn']:
            self.assertIn(key, keys)
        self.assertEqual(len(re.findall(r'\bt\b', keys)), 1)
        self.assertIn('t move selected item to Trash', keys)
        self.assertEqual(result.stderr, '')


class ScanAcceptance(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='clearing-accept-')
        self.path = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def assert_scan(self):
        actual = scan(self.path)
        for key, value in expected(self.path).items():
            self.assertEqual(actual[key], value, key)
        return actual

    def test_allocated_bytes_and_counts(self):
        (self.path / 'one').mkdir()
        (self.path / 'one' / 'data').write_bytes(b'a' * 12000)
        (self.path / 'tiny').write_bytes(b'abc')
        self.assert_scan()

    def test_sparse_file_uses_allocated_blocks(self):
        with (self.path / 'sparse').open('wb') as f:
            f.seek(128 * 1024 * 1024)
            f.write(b'x')
        actual = self.assert_scan()
        self.assertLess(actual['bytes'], actual['apparent_bytes'] // 10)

    def test_hardlinks_count_once_across_directories(self):
        for name in ['a', 'b']:
            (self.path / name).mkdir()
        (self.path / 'a/data').write_bytes(b'x' * 65536)
        os.link(self.path / 'a/data', self.path / 'b/other')
        for _ in range(3):
            self.assert_scan()

    def test_symlink_targets_and_cycles_are_not_walked(self):
        os.symlink('/', self.path / 'outside')
        os.symlink('.', self.path / 'cycle')
        os.symlink('does-not-exist', self.path / 'broken')
        actual = self.assert_scan()
        self.assertEqual(actual['files'], 3)
        self.assertEqual(actual['directories'], 1)

    def test_empty_directory(self):
        self.assert_scan()

    def test_unicode_and_control_filenames(self):
        for name in ['日本語', 'école', 'line\nbreak', 'escape\x1b[31mred', 'tab\tname']:
            (self.path / name).write_bytes(b'x' * 4096)
        self.assert_scan()
        snapshot = subprocess.run([str(BINARY), '--snapshot', 'overview', str(self.path)], capture_output=True, timeout=30)
        self.assertEqual(snapshot.returncode, 0, snapshot.stderr)
        self.assertNotIn(b'escape\x1b[31mred', snapshot.stdout)

    def test_non_utf8_root_and_child_paths(self):
        directory = self.path / os.fsdecode(b'root-\xff')
        directory.mkdir()
        (directory / os.fsdecode(b'file-\xfe')).write_bytes(b'x' * 8192)
        actual = scan(directory)
        for key, value in expected(directory).items():
            self.assertEqual(actual[key], value, key)
        self.assertIsInstance(actual['path'], str)

    def test_permission_error_is_reported(self):
        locked = self.path / 'locked'
        locked.mkdir()
        (locked / 'secret').write_bytes(b'x' * 4096)
        locked.chmod(0)
        try:
            if os.geteuid() == 0:
                self.skipTest('root bypasses directory permissions')
            self.assertEqual(self.assert_scan()['errors'], 1)
        finally:
            locked.chmod(0o700)

    def test_missing_path_fails_cleanly(self):
        path = self.path / 'missing'
        result = subprocess.run([str(BINARY), '--scan', str(path)], text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 1)
        self.assertIn(f'clearing: {path}: No such file or directory', result.stderr)
        self.assertEqual(result.stdout, '')

    def test_file_root_is_rejected(self):
        path = self.path / 'a file'
        path.write_bytes(b'data')
        for argument in [str(path), path.name]:
            with self.subTest(argument=argument):
                result = subprocess.run([str(BINARY), '--scan', '--json', argument], cwd=self.path, text=True, capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stderr, f'clearing: not a directory: {argument}\n')
                self.assertEqual(result.stdout, '')

    def test_unreadable_root_error_names_path(self):
        if os.geteuid() == 0:
            self.skipTest('root bypasses directory permissions')
        path = self.path / 'locked'
        path.mkdir()
        path.chmod(0)
        try:
            result = subprocess.run([str(BINARY), '--scan', str(path)], text=True, capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 1)
            self.assertIn(f'clearing: {path}: Permission denied', result.stderr)
            self.assertEqual(result.stdout, '')
        finally:
            path.chmod(0o700)

    def test_nonterminal_requires_scan_mode(self):
        result = subprocess.run([str(BINARY), str(self.path)], capture_output=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b'terminal', result.stderr)

    def test_small_terminal_sizes_do_not_panic(self):
        for width, height in [(1, 1), (2, 2), (40, 10), (59, 19), (60, 20), (80, 24)]:
            with self.subTest(width=width, height=height):
                result = subprocess.run([str(BINARY), '--snapshot', 'overview', '--width', str(width), '--height', str(height), str(self.path)], capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr)


class InteractionAcceptance(ScanAcceptance):
    # Only interaction methods are run from this class; scan checks run once above.
    def session(self):
        session = Session(self.path)
        self.addCleanup(session.close)
        return session

    def make_slow_scan_tree(self, entries=20000):
        # Enough directory entries to keep the worker busy while the PTY reads
        # the scan frame and sends Escape; no machine-specific tree or sleep.
        for index in range(entries):
            (self.path / f'file-{index:05d}').touch()

    def assert_idle_silence(self, session):
        session.read()  # Drain the rest of the frame after the heading matched.
        start = len(session.output)
        session.read(2, until=NEVER)  # Observe the full window, including empty frames.
        self.assertEqual(bytes(session.output[start:]), b'', 'idle screen wrote terminal bytes')

    def test_idle_silence_then_selection_and_resize(self):
        for name, size in [('Alpha', 32768), ('Zulu', 16384)]:
            directory = self.path / name
            directory.mkdir()
            (directory / f'inside-{name}').write_bytes(b'x' * size)
        session = self.session()
        self.assertIn(IDLE, ANSI.sub(b'', session.output), 'initial scan did not finish')
        self.assert_idle_silence(session)

        start = len(session.output)
        session.send(b'j')
        self.assertIn(b'Zulu', ANSI.sub(b'', session.output[start:]), 'selection did not redraw')
        start = len(session.output)
        session.send(b'\r')
        self.assertIn(b'inside-Zulu', ANSI.sub(b'', session.output[start:]), 'selection did not move')

        start = len(session.output)
        fcntl.ioctl(session.master, termios.TIOCSWINSZ, struct.pack('HHHH', 15, 50, 0, 0))
        session.read(1)
        self.assertIn(b'Resize to at least', ANSI.sub(b'', session.output[start:]))
        start = len(session.output)
        fcntl.ioctl(session.master, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 120, 0, 0))
        session.read(1)
        resized = bytes(session.output[start:])
        self.assertIn(b'ALLOCATED IN THIS VIEW', ANSI.sub(b'', resized), 'new width was not rendered')
        self.assertRegex(resized, rb'\x1b\[39;\d+H[^\n]*choose', 'footer did not move to the new height')
        self.assert_idle_silence(session)

    def test_after_delete_wait_is_silent_and_rescan_responds(self):
        root = self.path / 'root'
        root.mkdir()
        (root / 'candidate').write_bytes(b'x' * 32768)
        session = Session(root)
        self.addCleanup(session.close)
        moved = self.path / 'moved'
        root.rename(moved)
        session.send(b'd')
        start = len(session.output)
        session.send(b'delete\r', 2, until=b'Rescan failed:')
        self.assertIn(b'Rescan failed:', ANSI.sub(b'', session.output[start:]))
        self.assert_idle_silence(session)
        moved.rename(root)
        start = len(session.output)
        session.send(b'r', 2, until=IDLE)
        self.assertIn(IDLE, ANSI.sub(b'', session.output[start:]), 'retry did not restore browsing')
        self.assertTrue((root / 'candidate').exists())

    def test_scan_counter_advances_without_input(self):
        self.make_slow_scan_tree(200000)
        session = Session(self.path, until=b'entries scanned')
        self.addCleanup(session.close)
        self.assertNotIn(IDLE, ANSI.sub(b'', session.output), 'scan finished before progress was observed')
        session.read(10, until=IDLE)
        # Keep cursor moves but strip colour codes: ratatui updates only the
        # changed digits, so subsequent frames need not repeat "entries scanned".
        output = re.sub(rb'\x1b\[[0-9;]*m', b'', bytes(session.output))
        updates = re.findall(rb'\x1b\[4;\d+H *(\d+)', output.split(IDLE)[0])
        self.assertGreater(len(updates), 1, 'scan counter never redrew while scanning')
        self.assertTrue(any(int(value) > 0 for value in updates[1:]), 'scan counter did not advance')
        self.assertIn(IDLE, ANSI.sub(b'', session.output), 'scan did not finish')

    def test_first_scan_escape_label_and_exit(self):
        self.make_slow_scan_tree()
        session = Session(self.path, until=b'Esc ')
        self.addCleanup(session.close)
        screen = ANSI.sub(b'', session.output)
        self.assertIn(b'Esc quit', screen)
        self.assertNotIn(b'Esc cancel', screen)
        self.assertNotIn(IDLE, screen, 'first scan finished before Escape could be tested')
        session.send(b'\x1b')
        code = session.reap(2)
        if code is not None:
            session.closed = True
            os.close(session.master)
        self.assertEqual(code, 0, 'Escape during the first scan did not quit cleanly')

    def test_rescan_escape_label_and_retained_results(self):
        session = self.session()
        self.make_slow_scan_tree()
        start = len(session.output)
        session.send(b'r', 2, until=b'Esc ')
        screen = ANSI.sub(b'', session.output[start:])
        self.assertIn(b'Esc cancel', screen)
        self.assertNotIn(b'Esc quit', screen)
        start = len(session.output)
        session.send(b'\x1b', 2, until=b'Rescan cancelled; previous results retained')
        self.assertIn(b'Rescan cancelled; previous results retained', ANSI.sub(b'', session.output[start:]))
        self.assertIsNone(session.reap(0.1), 'Escape during a rescan exited the app')

    def test_cancel_and_wrong_confirmation_preserve_data(self):
        target = self.path / 'candidate.bin'
        target.write_bytes(b'x' * 32768)
        session = self.session()
        session.send(b'd')
        session.send(b'\r', until=NEVER)
        self.assertTrue(target.exists(), 'Enter without exact confirmation deleted a file')
        session.send(b'wrong\r', until=NEVER)
        self.assertTrue(target.exists(), 'wrong confirmation deleted a file')
        session.send(b'\x1b')
        self.assertTrue(target.exists())

    def test_confirmed_file_delete(self):
        target = self.path / 'candidate.bin'
        target.write_bytes(b'x' * 32768)
        session = self.session()
        session.send(b'd')
        session.send(b'delete\r', 0.75, until=IDLE)
        self.assertFalse(target.exists(), 'exact confirmed deletion did not remove selected file')
        self.assertTrue(self.path.exists())

    def test_single_delete_updates_total_and_keeps_row_without_scan(self):
        (self.path / 'largest').write_bytes(b'x' * 32768)
        target = self.path / 'candidate'
        target.write_bytes(b'x' * 16384)
        for index in range(30):
            (self.path / f'keep-{index:02d}').write_bytes(b'x' * 4096)
        allocated = target.stat().st_blocks * 512
        session = self.session()
        session.send(b'j')  # candidate is row 02; keep-00 will take its place.
        session.read()

        def row(number):
            # Replay absolute cursor writes, including partial changed cells.
            # This fixture and the UI use only single-column characters.
            output = re.sub(rb'\x1b\[[0-9;]*m', b'', bytes(session.output))
            cells = [' '] * 100
            for y, x, text in re.findall(rb'\x1b\[(\d+);(\d+)H([^\x1b]*)', output):
                if int(y) == number:
                    start = int(x) - 1
                    value = text.decode('utf-8')[:100 - start]
                    cells[start:start + len(value)] = value
            return ''.join(cells)

        def header_total():
            total = row(2)[73:].strip()
            self.assertRegex(total, r'^\d+\.\d+ KiB$')
            return float(total.split()[0])

        before_total = header_total()
        self.assertTrue(row(11)[52:].startswith('02 candidate'))
        self.assertTrue(row(29)[5:].startswith('candidate'))
        session.send(b'd')
        start = len(session.output)
        session.send(b'delete\r', 2, until=IDLE)
        session.read()  # Finish the frame after its heading matched.
        after = ANSI.sub(b'', session.output[start:])
        self.assertFalse(target.exists())
        self.assertNotIn(b'Reading disk allocation', after)
        self.assertNotIn(b'entries scanned', after)
        self.assertEqual(before_total - header_total(), allocated / 1024)
        self.assertTrue(row(11)[52:].startswith('02 keep-00'))
        self.assertTrue(row(29)[5:].startswith('keep-00'), 'selection did not stay on row 02')

    def test_drill_back_and_delete_directory_without_following_symlink(self):
        target = self.path / 'candidate'
        target.mkdir()
        (target / 'data').write_bytes(b'x' * 32768)
        with tempfile.TemporaryDirectory(prefix='clearing-outside-') as other:
            outside = Path(other) / 'must-survive'
            outside.write_bytes(b'keep')
            os.symlink(other, target / 'outside')
            session = self.session()
            session.send(b'\r')
            session.send(b'\x7f')
            session.send(b'd')
            session.send(b'delete\r', 0.75, until=IDLE)
            self.assertFalse(target.exists(), 'back did not restore original directory selection')
            self.assertEqual(outside.read_bytes(), b'keep')

    def test_confirmed_recursive_delete_can_be_stopped(self):
        target = self.path / 'candidate'
        target.mkdir()
        for index in range(5000):
            (target / f'file-{index:05d}').touch()
        session = self.session()
        session.send(b'd')
        session.send(b'delete\r\x1b', 1.0, until=IDLE)
        self.assertTrue(target.exists(), 'Escape did not stop the recursive deletion')
        self.assertGreater(sum(1 for _ in target.iterdir()), 0, 'all entries removed despite cancellation')
        self.assertTrue(self.path.exists())

    def test_replaced_selection_is_refused(self):
        target = self.path / 'candidate.bin'
        target.write_bytes(b'old' * 32768)
        session = self.session()
        target.rename(self.path / 'saved-original')
        target.write_bytes(b'replacement')
        session.send(b'd')
        session.send(b'delete\r', 0.75, until=IDLE)
        self.assertEqual(target.read_bytes(), b'replacement')
        self.assertTrue((self.path / 'saved-original').exists())

    def test_replaced_parent_symlink_is_refused(self):
        target = self.path / 'candidate'
        target.mkdir()
        (target / 'data').write_bytes(b'old' * 32768)
        with tempfile.TemporaryDirectory(prefix='clearing-outside-') as other:
            outside = Path(other) / 'data'
            outside.write_bytes(b'keep')
            session = self.session()
            session.send(b'\r')
            target.rename(self.path / 'saved-original')
            os.symlink(other, target)
            session.send(b'd')
            session.send(b'delete\r', 0.75, until=IDLE)
            self.assertEqual(outside.read_bytes(), b'keep')
            self.assertTrue((self.path / 'saved-original' / 'data').exists())


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=BINARY)
    parser.add_argument('--scan-only', action='store_true')
    args = parser.parse_args()
    BINARY = args.binary.resolve()
    if (ROOT / '.runtime/FROZEN').exists():
        raise SystemExit('Frozen at user deadline; refusing further work.')
    original_hash = hashlib.sha256(BINARY.read_bytes()).hexdigest()
    print(f'Release SHA-256: {original_hash}', flush=True)
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(CommandLineAcceptance)
    suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(ScanAcceptance))
    if not args.scan_only:
        for name in InteractionAcceptance.__dict__:
            if name.startswith('test_'):
                suite.addTest(InteractionAcceptance(name))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    unchanged = hashlib.sha256(BINARY.read_bytes()).hexdigest() == original_hash
    if not unchanged:
        print('Binary changed during testing; this run is invalid.')
    raise SystemExit(not (result.wasSuccessful() and unchanged))
