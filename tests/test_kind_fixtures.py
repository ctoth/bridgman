from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest
import sympy as sp

from bridgman import KindRegistry, OperationRule, QuantityKind, explain_expr_kinds


FIXTURE_DIR = Path(__file__).resolve().parent / "fixtures"
REQUIRED_TOP_LEVEL_KEYS = {"kinds", "rules", "valid_equations", "invalid_equations"}


def _load_fixture(name: str) -> dict[str, Any]:
    data = json.loads((FIXTURE_DIR / name).read_text(encoding="utf-8"))
    _validate_fixture_schema(data)
    return data


def _validate_fixture_schema(data: dict[str, Any]) -> None:
    missing = REQUIRED_TOP_LEVEL_KEYS - set(data)
    if missing:
        raise ValueError(f"fixture missing required keys: {sorted(missing)}")

    seen_kinds: set[str] = set()
    for kind in data["kinds"]:
        if kind["name"] in seen_kinds:
            raise ValueError(f"duplicate kind: {kind['name']}")
        seen_kinds.add(kind["name"])
        if not isinstance(kind["dimensions"], dict):
            raise ValueError(f"kind dimensions must be a mapping: {kind['name']}")

    for rule in data["rules"]:
        for field in ("left", "op", "right", "result"):
            if field not in rule:
                raise ValueError(f"rule missing {field}")


def _registry_from_fixture(data: dict[str, Any]) -> KindRegistry:
    return KindRegistry(
        kinds=[
            QuantityKind(kind["name"], kind["dimensions"])
            for kind in data["kinds"]
        ],
        rules=[
            OperationRule(
                rule["left"],
                rule["op"],
                rule["right"],
                rule["result"],
                commutative=rule.get("commutative", False),
                rationale=rule.get("rationale"),
            )
            for rule in data["rules"]
        ],
    )


def _parse_equation(text: str, kind_map: dict[str, str]) -> sp.Expr:
    locals_map = {"Eq": sp.Eq}
    locals_map.update({symbol: sp.Symbol(symbol) for symbol in kind_map})
    return sp.sympify(text, locals=locals_map)


@pytest.mark.parametrize("fixture_name", ("kinds_mechanics.yml", "kinds_collisions.yml"))
def test_kind_fixture_schema_loads(fixture_name: str) -> None:
    data = _load_fixture(fixture_name)

    assert data["kinds"]
    assert _registry_from_fixture(data)


@pytest.mark.parametrize("fixture_name", ("kinds_mechanics.yml", "kinds_collisions.yml"))
def test_kind_fixture_equations_validate(fixture_name: str) -> None:
    data = _load_fixture(fixture_name)
    registry = _registry_from_fixture(data)

    for case in data["valid_equations"]:
        result = explain_expr_kinds(
            _parse_equation(case["expr"], case["kind_map"]),
            registry=registry,
            kind_map=case["kind_map"],
        )
        assert result.ok, result.reason

    for case in data["invalid_equations"]:
        result = explain_expr_kinds(
            _parse_equation(case["expr"], case["kind_map"]),
            registry=registry,
            kind_map=case["kind_map"],
        )
        assert not result.ok
        assert case["reason"] in result.reason


def test_fixture_schema_rejects_missing_keys() -> None:
    with pytest.raises(ValueError, match="missing required keys"):
        _validate_fixture_schema({"kinds": []})


def test_fixture_schema_rejects_duplicate_kinds() -> None:
    with pytest.raises(ValueError, match="duplicate kind"):
        _validate_fixture_schema(
            {
                "kinds": [
                    {"name": "Length", "dimensions": {"L": 1}},
                    {"name": "Length", "dimensions": {"L": 1}},
                ],
                "rules": [],
                "valid_equations": [],
                "invalid_equations": [],
            }
        )
