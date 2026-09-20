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
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time
import unittest

ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT / 'target/release/spacemap'


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
    def __init__(self, path, cols=100, rows=34):
        self.pid, self.master = pty.fork()
        if not self.pid:
            os.environ.update(TERM='xterm-256color', COLORTERM='truecolor')
            os.execv(str(BINARY), [str(BINARY), str(path)])
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
        self.output = bytearray()
        self.closed = False
        self.read(1)

    def read(self, duration=0.25):
        until = time.monotonic() + duration
        while time.monotonic() < until:
            if not select.select([self.master], [], [], max(0, until - time.monotonic()))[0]:
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
        return bytes(self.output)

    def send(self, data, pause=0.25):
        os.write(self.master, data)
        return self.read(pause)

    def close(self):
        if self.closed:
            return
        self.closed = True
        try:
            self.send(b'\x03', 0.2)
            for _ in range(10):
                pid, status = os.waitpid(self.pid, os.WNOHANG)
                if pid:
                    return os.waitstatus_to_exitcode(status)
                time.sleep(0.05)
            os.kill(self.pid, signal.SIGTERM)
            os.waitpid(self.pid, 0)
        finally:
            os.close(self.master)


class ScanAcceptance(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='spacemap-accept-')
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
        result = subprocess.run([str(BINARY), '--scan', str(self.path / 'missing')], capture_output=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b'spacemap:', result.stderr)

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

    def test_cancel_and_wrong_confirmation_preserve_data(self):
        target = self.path / 'candidate.bin'
        target.write_bytes(b'x' * 32768)
        session = self.session()
        session.send(b'd')
        session.send(b'\r')
        self.assertTrue(target.exists(), 'Enter without exact confirmation deleted a file')
        session.send(b'wrong\r')
        self.assertTrue(target.exists(), 'wrong confirmation deleted a file')
        session.send(b'\x1b')
        self.assertTrue(target.exists())

    def test_confirmed_file_delete(self):
        target = self.path / 'candidate.bin'
        target.write_bytes(b'x' * 32768)
        session = self.session()
        session.send(b'd')
        session.send(b'delete\r', 0.75)
        self.assertFalse(target.exists(), 'exact confirmed deletion did not remove selected file')
        self.assertTrue(self.path.exists())

    def test_drill_back_and_delete_directory_without_following_symlink(self):
        target = self.path / 'candidate'
        target.mkdir()
        (target / 'data').write_bytes(b'x' * 32768)
        with tempfile.TemporaryDirectory(prefix='spacemap-outside-') as other:
            outside = Path(other) / 'must-survive'
            outside.write_bytes(b'keep')
            os.symlink(other, target / 'outside')
            session = self.session()
            session.send(b'\r')
            session.send(b'\x7f')
            session.send(b'd')
            session.send(b'delete\r', 0.75)
            self.assertFalse(target.exists(), 'back did not restore original directory selection')
            self.assertEqual(outside.read_bytes(), b'keep')

    def test_confirmed_recursive_delete_can_be_stopped(self):
        target = self.path / 'candidate'
        target.mkdir()
        for index in range(5000):
            (target / f'file-{index:05d}').touch()
        session = self.session()
        session.send(b'd')
        session.send(b'delete\r\x1b', 1.0)
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
        session.send(b'delete\r', 0.75)
        self.assertEqual(target.read_bytes(), b'replacement')
        self.assertTrue((self.path / 'saved-original').exists())

    def test_replaced_parent_symlink_is_refused(self):
        target = self.path / 'candidate'
        target.mkdir()
        (target / 'data').write_bytes(b'old' * 32768)
        with tempfile.TemporaryDirectory(prefix='spacemap-outside-') as other:
            outside = Path(other) / 'data'
            outside.write_bytes(b'keep')
            session = self.session()
            session.send(b'\r')
            target.rename(self.path / 'saved-original')
            os.symlink(other, target)
            session.send(b'd')
            session.send(b'delete\r', 0.75)
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
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ScanAcceptance)
    if not args.scan_only:
        for name in InteractionAcceptance.__dict__:
            if name.startswith('test_'):
                suite.addTest(InteractionAcceptance(name))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    unchanged = hashlib.sha256(BINARY.read_bytes()).hexdigest() == original_hash
    if not unchanged:
        print('Binary changed during testing; this run is invalid.')
    raise SystemExit(not (result.wasSuccessful() and unchanged))
