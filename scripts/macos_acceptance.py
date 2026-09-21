#!/usr/bin/env python3
"""Native macOS Trash checks, using only uniquely named disposable test items."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
import uuid

import acceptance


@unittest.skipUnless(sys.platform == 'darwin', 'requires macOS native Trash')
class NativeTrashAcceptance(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='clearing-native-')
        self.base = Path(self.temporary.name).resolve()
        self.tree = self.base / 'tree'
        self.tree.mkdir()
        self.addCleanup(self.temporary.cleanup)
        self.trash = Path.home() / '.Trash'
        self.items = []
        self.addCleanup(self.restore_owned_items)

    def item(self, content=b'Native Trash test data'):
        path = self.tree / ('clearing-test-' + uuid.uuid4().hex)
        path.write_bytes(content)
        self.items.append(path)
        return path

    def restore_owned_items(self):
        for original in self.items:
            trashed = self.trash / original.name
            if os.path.lexists(trashed) and not os.path.lexists(original):
                trashed.rename(original)

    def session(self):
        session = acceptance.Session(self.tree)
        self.addCleanup(session.close)
        return session

    def test_native_trash_moves_and_restores_the_confirmed_file(self):
        path = self.item()
        content = path.read_bytes()
        session = self.session()
        session.send(b't\rwrong\r')
        self.assertTrue(path.exists(), 'wrong confirmation moved an item')
        session.send(b'\x1b')
        session.send(b'ttrash\r', 2.0)
        self.assertFalse(path.exists(), 'confirmed native Trash did not move the item')
        trashed = self.trash / path.name
        self.assertEqual(trashed.read_bytes(), content, 'native Trash lost the payload')
        trashed.rename(path)
        self.assertEqual(path.read_bytes(), content, 'restoring from Trash lost the payload')

    def test_native_trash_keeps_symlink_target_intact(self):
        target = self.base / 'outside-target'
        target.write_bytes(b'keep this target')
        link = self.tree / ('clearing-test-' + uuid.uuid4().hex)
        link.symlink_to(target)
        self.items.append(link)
        session = self.session()
        session.send(b'ttrash\r', 2.0)
        self.assertFalse(link.is_symlink())
        self.assertEqual(target.read_bytes(), b'keep this target')
        trashed = self.trash / link.name
        self.assertTrue(trashed.is_symlink(), 'native Trash replaced the symlink')
        self.assertEqual(os.readlink(trashed), str(target))


if __name__ == '__main__':
    unittest.main(verbosity=2)
