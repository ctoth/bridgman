from __future__ import annotations

from pathlib import Path


WORKSTREAM = Path(__file__).resolve().parents[1] / "PHYSgen_WORKSTREAM.md"


def _line_number(lines: list[str], needle: str) -> int:
    for index, line in enumerate(lines, start=1):
        if line.strip() == needle:
            return index
    raise AssertionError(f"missing workstream heading: {needle}")


def test_physgen_workstream_phases_are_dependency_ordered() -> None:
    lines = WORKSTREAM.read_text(encoding="utf-8").splitlines()
    headings = (
        "## Phase 1: Fix Current Symbolic Semantics",
        "## Phase 2: Add the Physgen Collision Corpus",
        "## Phase 3: Add Kind and Operation Models",
        "## Phase 4: Kind-Aware Symbolic Checking",
        "## Phase 5: Add Explanation Results",
        "## Phase 6: Declarative Physics Fixtures",
        "## Phase 7: Documentation and Release Surface",
        "## Phase 8: Propstore Integration Contract Tests",
    )

    phase_lines = [_line_number(lines, heading) for heading in headings]

    assert phase_lines == sorted(phase_lines)


def test_propstore_contract_precedes_propstore_contract_tests() -> None:
    lines = WORKSTREAM.read_text(encoding="utf-8").splitlines()

    contract_line = _line_number(lines, "## Propstore Consumer Contract")
    phase_8_line = _line_number(lines, "## Phase 8: Propstore Integration Contract Tests")

    assert contract_line < phase_8_line
