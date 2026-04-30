from __future__ import annotations

from pathlib import Path


def test_source_uses_theta_spelling_not_theta_glyph() -> None:
    src_root = Path("src")
    forbidden = {chr(0x0398), chr(0x03B8)}

    offenders: list[Path] = []
    for path in src_root.rglob("*.py"):
        if any(glyph in path.read_text(encoding="utf-8") for glyph in forbidden):
            offenders.append(path)

    assert offenders == []
