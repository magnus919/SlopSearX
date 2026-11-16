from __future__ import annotations

import os
import tempfile
import threading
import unittest
from pathlib import Path
from unittest import mock

from scripts import coverage_atomic_io


class AtomicPublicationTests(unittest.TestCase):
    def test_final_name_is_invisible_until_payload_is_complete(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "request.json"
            entered_write = threading.Event()
            allow_write = threading.Event()
            real_fdopen = os.fdopen

            class PausingStream:
                def __init__(self, stream):
                    self.stream = stream

                def __enter__(self):
                    return self

                def __exit__(self, *args):
                    return self.stream.__exit__(*args)

                def write(self, raw):
                    entered_write.set()
                    if not allow_write.wait(2):
                        raise TimeoutError("test write barrier timed out")
                    return self.stream.write(raw)

                def flush(self):
                    return self.stream.flush()

                def fileno(self):
                    return self.stream.fileno()

            def pausing_fdopen(*args, **kwargs):
                return PausingStream(real_fdopen(*args, **kwargs))

            failures: list[BaseException] = []

            def publish():
                try:
                    coverage_atomic_io.write_new(target, b'{"complete":true}')
                except BaseException as exc:
                    failures.append(exc)

            with mock.patch.object(coverage_atomic_io.os, "fdopen", side_effect=pausing_fdopen):
                worker = threading.Thread(target=publish, daemon=True)
                worker.start()
                self.assertTrue(entered_write.wait(1), "publisher did not reach staged payload write")
                try:
                    self.assertFalse(target.exists())
                finally:
                    allow_write.set()
                worker.join(timeout=2)

            self.assertFalse(worker.is_alive())
            self.assertEqual(failures, [])
            self.assertEqual(target.read_bytes(), b'{"complete":true}')
            self.assertEqual(target.stat().st_mode & 0o777, 0o600)
            self.assertFalse((Path(directory) / ".request.json.claim").exists())

    def test_collision_and_failed_write_keep_create_once_claim_private(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "request.json"
            target.write_bytes(b"original")
            with self.assertRaises(FileExistsError):
                coverage_atomic_io.write_new(target, b"replacement")
            self.assertEqual(target.read_bytes(), b"original")
            self.assertFalse((Path(directory) / ".request.json.claim").exists())

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "request.json"
            with mock.patch.object(coverage_atomic_io.os, "fsync", side_effect=OSError("injected fsync failure")):
                with self.assertRaises(OSError):
                    coverage_atomic_io.write_new(target, b"partial must stay private")
            self.assertFalse(target.exists())
            claim = Path(directory) / ".request.json.claim"
            self.assertTrue(claim.exists())
            self.assertEqual(claim.stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError):
                coverage_atomic_io.write_new(target, b"no retry")

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "request.json"
            real_fdopen = os.fdopen

            class FailingStream:
                def __init__(self, stream):
                    self.stream = stream

                def __enter__(self):
                    return self

                def __exit__(self, *args):
                    return self.stream.__exit__(*args)

                def write(self, _raw):
                    raise OSError("injected payload write failure")

                def flush(self):
                    return self.stream.flush()

                def fileno(self):
                    return self.stream.fileno()

            with mock.patch.object(
                coverage_atomic_io.os,
                "fdopen",
                side_effect=lambda *args, **kwargs: FailingStream(real_fdopen(*args, **kwargs)),
            ):
                with self.assertRaisesRegex(OSError, "payload write failure"):
                    coverage_atomic_io.write_new(target, b"must not publish")
            self.assertFalse(target.exists())
            claim = Path(directory) / ".request.json.claim"
            self.assertTrue(claim.exists())
            self.assertEqual(claim.stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError):
                coverage_atomic_io.write_new(target, b"no retry")

    def test_symlink_claim_and_final_are_not_followed_or_replaced(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            victim = root / "victim"
            victim.write_bytes(b"safe")
            target = root / "request.json"
            target.symlink_to(victim)
            with self.assertRaises(FileExistsError):
                coverage_atomic_io.write_new(target, b"payload")
            self.assertEqual(victim.read_bytes(), b"safe")

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            victim = root / "victim"
            victim.write_bytes(b"safe")
            target = root / "request.json"
            (root / ".request.json.claim").symlink_to(victim)
            with self.assertRaises(OSError):
                coverage_atomic_io.write_new(target, b"payload")
            self.assertFalse(target.exists())
            self.assertEqual(victim.read_bytes(), b"safe")

    def test_destination_created_after_precheck_leaves_no_claim(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "request.json"
            real_link = os.link

            def create_collision_then_link(src, dst, **kwargs):
                if dst == target.name:
                    descriptor = os.open(dst, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=kwargs["dst_dir_fd"])
                    try:
                        os.write(descriptor, b"other writer")
                    finally:
                        os.close(descriptor)
                return real_link(src, dst, **kwargs)

            with mock.patch.object(coverage_atomic_io.os, "link", side_effect=create_collision_then_link):
                with self.assertRaises(FileExistsError):
                    coverage_atomic_io.write_new(target, b"our payload")
            self.assertEqual(target.read_bytes(), b"other writer")
            self.assertFalse((Path(directory) / ".request.json.claim").exists())


if __name__ == "__main__":
    unittest.main()
