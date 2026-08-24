"""Verify that a Bridgman sdist carries the license named by its metadata."""

from __future__ import annotations

import argparse
from email.parser import BytesParser
from pathlib import Path, PurePosixPath
import tarfile


def check_sdist(sdist: Path, project_root: Path) -> None:
    expected_license = (project_root / "LICENSE").read_bytes()
    with tarfile.open(sdist, "r:gz") as archive:
        members = {PurePosixPath(member.name): member for member in archive.getmembers()}
        roots = {path.parts[0] for path in members if path.parts}
        if len(roots) != 1:
            raise ValueError(f"expected one sdist root, found {sorted(roots)}")

        root = next(iter(roots))
        metadata_path = PurePosixPath(root, "PKG-INFO")
        metadata_file = archive.extractfile(members[metadata_path])
        if metadata_file is None:
            raise ValueError("PKG-INFO is not a regular file")
        metadata = BytesParser().parsebytes(metadata_file.read())
        license_files = metadata.get_all("License-File", [])
        if license_files != ["LICENSE"]:
            raise ValueError(f"unexpected License-File metadata: {license_files!r}")

        license_path = PurePosixPath(root, "LICENSE")
        license_file = archive.extractfile(members[license_path])
        if license_file is None:
            raise ValueError("LICENSE is not a regular file")
        actual_license = license_file.read()
        if actual_license != expected_license:
            raise ValueError("sdist LICENSE does not match the repository LICENSE")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("sdist", type=Path)
    parser.add_argument("--project-root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    check_sdist(args.sdist, args.project_root)
    print(f"verified LICENSE and metadata in {args.sdist}")


if __name__ == "__main__":
    main()
