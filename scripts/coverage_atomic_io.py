"""Durable publication of private, create-once files."""

from __future__ import annotations

import errno
import os
from pathlib import Path


def write_new(path: Path, raw: bytes) -> None:
    """Publish complete bytes under *path* without ever exposing a partial file.

    The deterministic hidden claim reserves the destination across crashes and
    failed writes. A hard link atomically publishes the finished inode without
    replacing an existing destination.
    """
    parent_fd = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0))
    name = path.name
    claim_name = f".{name}.claim"
    try:
        try:
            os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise FileExistsError(errno.EEXIST, "file exists", str(path))

        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
        descriptor = os.open(claim_name, flags, 0o600, dir_fd=parent_fd)
        try:
            os.fsync(parent_fd)
            with os.fdopen(descriptor, "wb", closefd=False) as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
        finally:
            os.close(descriptor)

        try:
            os.link(claim_name, name, src_dir_fd=parent_fd, dst_dir_fd=parent_fd, follow_symlinks=False)
        except FileExistsError:
            os.unlink(claim_name, dir_fd=parent_fd)
            os.fsync(parent_fd)
            raise
        os.fsync(parent_fd)
        os.unlink(claim_name, dir_fd=parent_fd)
        os.fsync(parent_fd)
    finally:
        os.close(parent_fd)
