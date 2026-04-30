from __future__ import annotations

import ast
from pathlib import Path

import pytest

import bridgman


def test_verify_equation_cannot_be_imported() -> None:
    with pytest.raises(ImportError):
        from bridgman import verify_equation  # noqa: F401


def test_verify_equation_absent_from_public_surface() -> None:
    assert "verify_equation" not in dir(bridgman)
    assert "verify_equation" not in bridgman.__all__


def test_verify_equation_has_no_source_definition_or_calls() -> None:
    offenders: list[str] = []
    for path in Path("src").rglob("*.py"):
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        for node in ast.walk(tree):
            if isinstance(node, ast.FunctionDef) and node.name == "verify_equation":
                offenders.append(f"{path}:def")
            if isinstance(node, ast.Call):
                func = node.func
                if isinstance(func, ast.Name) and func.id == "verify_equation":
                    offenders.append(f"{path}:call")
                if isinstance(func, ast.Attribute) and func.attr == "verify_equation":
                    offenders.append(f"{path}:call")

    assert offenders == []
