#!/usr/bin/env python3
"""Independent PTY tests. Real GIO uses an isolated, disposable desktop Trash."""
import argparse
import configparser
import hashlib
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch
from urllib.parse import unquote_to_bytes

import acceptance

ROOT = Path(__file__).resolve().parent.parent


class CollectorAcceptance(unittest.TestCase):
    def setUp(self):
        (ROOT / '.runtime').mkdir(exist_ok=True)
        self.tmp = tempfile.TemporaryDirectory(prefix='collector-accept-', dir=ROOT / '.runtime')
        self.base = Path(self.tmp.name)
        self.tree = self.base / 'tree'
        self.data = self.base / 'desktop-data'
        self.tree.mkdir()
        self.data.mkdir()
        self.addCleanup(self.tmp.cleanup)

    def file(self, name, size=32768, content=b'A'):
        path = self.tree / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content * size)
        return path

    def session(self, **env):
        with patch.dict(os.environ, {'XDG_DATA_HOME': str(self.data), **env}):
            session = acceptance.Session(self.tree)
        self.addCleanup(session.close)
        return session

    def trash_records(self):
        records = {}
        for info in (self.data / 'Trash/info').glob('*.trashinfo'):
            text = configparser.ConfigParser(interpolation=None)
            text.read(info)
            original = os.fsdecode(unquote_to_bytes(text['Trash Info']['Path']))
            records[original] = self.data / 'Trash/files' / info.name.removesuffix('.trashinfo')
        return records

    def commit(self, session):
        session.send(b'c')
        session.send(b't')
        session.send(b'trash\r', 1.0)

    def test_cross_directory_collection_trashes_both_with_restore_metadata(self):
        a = self.file('Alpha/same.bin', 32768, b'A')
        b = self.file('Beta/same.bin', 16384, b'B')
        session = self.session()
        session.send(b'\r \x7f')
        session.send(b'j\r ')
        self.commit(session)
        self.assertFalse(a.exists())
        self.assertFalse(b.exists())
        records = self.trash_records()
        self.assertEqual(set(records), {str(a), str(b)})
        self.assertEqual(records[str(a)].read_bytes(), b'A' * 32768)
        self.assertEqual(records[str(b)].read_bytes(), b'B' * 16384)

    def test_cancel_and_wrong_confirmation_leave_everything_in_place(self):
        a = self.file('first.bin')
        b = self.file('second.bin', 16384)
        session = self.session()
        session.send(b' j c')
        session.send(b't\rwrong\r')
        self.assertTrue(a.exists() and b.exists())
        self.assertEqual(self.trash_records(), {})
        session.send(b'\x1b')
        session.send(b'\x1b')
        self.assertTrue(a.exists() and b.exists())

    def test_manual_rescan_preserves_collection(self):
        a = self.file('chosen.bin')
        session = self.session()
        session.send(b' ')
        session.send(b'r', 0.75)
        self.commit(session)
        self.assertFalse(a.exists())
        self.assertEqual(set(self.trash_records()), {str(a)})

    def test_removing_from_review_does_not_trash_it(self):
        a = self.file('keep.bin')
        session = self.session()
        session.send(b' c')
        session.send(b'\x7f')
        session.send(b'ttrash\r')
        self.assertEqual(a.read_bytes(), b'A' * 32768)
        self.assertEqual(self.trash_records(), {})

    def test_selected_parent_absorbs_collected_child(self):
        child = self.file('folder/child.bin')
        session = self.session()
        session.send(b'\r \x7f ')
        self.commit(session)
        records = self.trash_records()
        self.assertFalse(child.parent.exists())
        self.assertEqual(set(records), {str(child.parent)})
        self.assertEqual((records[str(child.parent)] / child.name).read_bytes(), b'A' * 32768)

    def test_covered_child_cannot_add_a_duplicate_trash_operation(self):
        child = self.file('folder/child.bin')
        session = self.session()
        session.send(b' \r ')
        self.commit(session)
        self.assertFalse(child.parent.exists())
        self.assertEqual(set(self.trash_records()), {str(child.parent)})

    def test_rescan_never_authorizes_a_replaced_collected_file(self):
        a = self.file('chosen.bin')
        session = self.session()
        session.send(b' ')
        saved = self.base / 'original'
        a.rename(saved)
        a.write_bytes(b'replacement')
        session.send(b'r', 0.75)
        self.commit(session)
        self.assertEqual(a.read_bytes(), b'replacement')
        self.assertEqual(saved.read_bytes(), b'A' * 32768)
        self.assertEqual(self.trash_records(), {})

    def test_swapped_parent_symlink_cannot_trash_an_outside_file(self):
        a = self.file('folder/chosen.bin')
        outside = self.base / 'outside'
        outside.mkdir()
        (outside / a.name).write_bytes(b'outside')
        session = self.session()
        session.send(b'\r ')
        a.parent.rename(self.base / 'original-folder')
        a.parent.symlink_to(outside, target_is_directory=True)
        self.commit(session)
        self.assertEqual((outside / a.name).read_bytes(), b'outside')
        self.assertTrue((self.base / 'original-folder' / a.name).exists())
        self.assertEqual(self.trash_records(), {})

    def test_trashing_a_symlink_preserves_its_target(self):
        outside = self.base / 'target'
        outside.write_bytes(b'outside')
        link = self.tree / 'link'
        link.symlink_to(outside)
        session = self.session()
        session.send(b' ')
        self.commit(session)
        self.assertFalse(link.is_symlink())
        self.assertEqual(outside.read_bytes(), b'outside')
        records = self.trash_records()
        self.assertTrue(records[str(link)].is_symlink())
        self.assertEqual(os.readlink(records[str(link)]), str(outside))

    def test_non_utf8_and_control_filename_is_trashed_by_original_bytes(self):
        a = self.file(os.fsdecode(b'odd-\xff-\n.bin'))
        session = self.session()
        session.send(b' ')
        self.commit(session)
        self.assertFalse(a.exists())
        self.assertEqual(self.trash_records()[str(a)].read_bytes(), b'A' * 32768)

    def test_permanent_deletion_of_another_item_preserves_collection(self):
        collected = self.file('collected.bin', 32768)
        deleted = self.file('permanently-deleted.bin', 16384)
        session = self.session()
        session.send(b' jd')
        session.send(b'delete\r', 0.75)
        self.assertFalse(deleted.exists())
        self.assertTrue(collected.exists())
        self.commit(session)
        self.assertFalse(collected.exists())
        self.assertEqual(set(self.trash_records()), {str(collected)})

    def test_failed_item_does_not_erase_other_collection_entries(self):
        a = self.file('stale.bin', 32768)
        b = self.file('valid.bin', 16384)
        session = self.session()
        session.send(b' j ')
        a.rename(self.base / 'original')
        a.write_bytes(b'replacement')
        self.commit(session)
        self.assertEqual(a.read_bytes(), b'replacement')
        self.assertFalse(b.exists(), 'a stale entry prevented the other confirmed item from being trashed')
        self.assertEqual(set(self.trash_records()), {str(b)})
        a.rename(self.base / 'replacement')
        (self.base / 'original').rename(a)
        self.commit(session)
        self.assertFalse(a.exists(), 'the failed entry was lost from the collection')
        self.assertEqual(set(self.trash_records()), {str(a), str(b)})
        self.assertEqual((self.base / 'replacement').read_bytes(), b'replacement')

    def test_missing_trash_backend_never_falls_back_to_permanent_delete(self):
        a = self.file('keep.bin')
        empty_path = self.base / 'empty-path'
        empty_path.mkdir()
        session = self.session(PATH=str(empty_path))
        session.send(b' ')
        self.commit(session)
        self.assertEqual(a.read_bytes(), b'A' * 32768)
        self.assertEqual(self.trash_records(), {})

    def test_cancel_between_moves_retains_unprocessed_items(self):
        paths = [self.file(f'item-{i}.bin', (3 - i) * 16384) for i in range(3)]
        tools = self.base / 'tools'
        tools.mkdir()
        receipt = self.base / 'first-move'
        wrapper = tools / 'gio'
        # Real GIO performs the move; the delay makes the inter-item cancel
        # deterministic without depending on directory size or CPU speed.
        wrapper.write_text(
            '#!/usr/bin/python3\n'
            'import os, pathlib, subprocess, sys, time\n'
            'result = subprocess.run(["/usr/bin/gio", *sys.argv[1:]])\n'
            'pathlib.Path(os.environ["COLLECTOR_TEST_RECEIPT"]).touch()\n'
            'time.sleep(1)\n'
            'sys.exit(result.returncode)\n'
        )
        wrapper.chmod(0o700)
        session = self.session(PATH=str(tools), COLLECTOR_TEST_RECEIPT=str(receipt))
        session.send(b' j j ct')
        session.send(b'trash\r', 0.05)
        until = time.monotonic() + 5
        while not receipt.exists() and time.monotonic() < until:
            session.read(0.05)
        self.assertTrue(receipt.exists(), 'the isolated trash backend was never invoked')
        session.send(b'\x1b', 1.5)
        self.assertEqual(sum(p.exists() for p in paths), 2)
        self.assertEqual(len(self.trash_records()), 1)
        self.assertTrue(self.tree.exists())
        session.send(b'c')
        session.send(b'ttrash\r', 3.0)
        self.assertFalse(any(p.exists() for p in paths), 'unprocessed items were lost from the collection')
        self.assertEqual(len(self.trash_records()), 3)

    def test_direct_trash_moves_only_the_highlighted_item(self):
        chosen = self.file('chosen.bin', 32768)
        kept = self.file('keep.bin', 16384)
        session = self.session()
        session.send(b't')
        session.send(b'trash\r', 1.0)
        self.assertFalse(chosen.exists())
        self.assertTrue(kept.exists())
        self.assertEqual(set(self.trash_records()), {str(chosen)})

    def test_direct_trash_preserves_an_unrelated_collection(self):
        collected = self.file('collected.bin', 32768)
        chosen = self.file('single.bin', 16384)
        session = self.session()
        session.send(b' jt')
        session.send(b'trash\r', 1.0)
        self.assertFalse(chosen.exists())
        self.assertTrue(collected.exists(), 'direct Trash included an unrelated collected item')
        self.assertEqual(set(self.trash_records()), {str(chosen)})
        self.commit(session)
        self.assertFalse(collected.exists(), 'direct Trash erased the existing collection')
        self.assertEqual(set(self.trash_records()), {str(chosen), str(collected)})

    def test_direct_trash_requires_exact_confirmation_and_can_cancel(self):
        chosen = self.file('keep.bin')
        session = self.session()
        session.send(b't\rwrong\r')
        self.assertTrue(chosen.exists())
        self.assertEqual(self.trash_records(), {})
        session.send(b'\x1b')
        self.assertTrue(chosen.exists())
        session.send(b'ttrash\x1b')
        self.assertTrue(chosen.exists())
        self.assertEqual(self.trash_records(), {})

    def test_direct_trash_rejects_a_replacement_after_confirmation_opens(self):
        chosen = self.file('chosen.bin')
        session = self.session()
        session.send(b't')
        chosen.rename(self.base / 'original')
        chosen.write_bytes(b'replacement')
        session.send(b'trash\r', 1.0)
        self.assertEqual(chosen.read_bytes(), b'replacement')
        self.assertEqual(self.trash_records(), {})

    def test_direct_trash_of_a_symlink_keeps_its_target(self):
        outside = self.base / 'target'
        outside.write_bytes(b'outside')
        link = self.tree / 'link'
        link.symlink_to(outside)
        session = self.session()
        session.send(b't')
        session.send(b'trash\r', 1.0)
        self.assertFalse(link.is_symlink())
        self.assertEqual(outside.read_bytes(), b'outside')
        self.assertTrue(self.trash_records()[str(link)].is_symlink())


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=acceptance.BINARY)
    args = parser.parse_args()
    acceptance.BINARY = args.binary.resolve()
    original_hash = hashlib.sha256(acceptance.BINARY.read_bytes()).hexdigest()
    print(f'Release SHA-256: {original_hash}', flush=True)
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(CollectorAcceptance)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    unchanged = hashlib.sha256(acceptance.BINARY.read_bytes()).hexdigest() == original_hash
    if not unchanged:
        print('Binary changed during testing; this run is invalid.')
    raise SystemExit(not (result.wasSuccessful() and unchanged))
