"""Python asks the Rust core for kind arithmetic; it keeps no rules of its own."""

from __future__ import annotations

import ast
import typing
from pathlib import Path

import pytest
import sympy as sp

import bridgman
from bridgman import (
    InvalidOperationRuleError,
    KindError,
    KindMismatchError,
    KindRegistry,
    MissingOperationRuleError,
    OperationRule,
    QuantityKind,
    kind_of_expr,
)
from bridgman.kinds import OperationName


LENGTH = {"L": 1}
TIME = {"T": 1}
FORCE = {"M": 1, "L": 1, "T": -2}
ENERGY = {"M": 1, "L": 2, "T": -2}

REMOVED_NAMES = {
    "operation_rule",
    "unique_kind_with_dimensions",
    "_rules",
    "_store_rule",
    "_kinds",
}


def _symbolic_kind(expr, registry: KindRegistry, kind_map: dict[str, str]) -> str | KindError:
    try:
        result = kind_of_expr(expr, registry=registry, kind_map=kind_map)
    except KindError as exc:
        return exc
    assert result is not None
    return result


def _core_kind(registry: KindRegistry, left: str, op: OperationName, right: str) -> str | KindError:
    try:
        return registry.result_kind(left, op, right)
    except KindError as exc:
        return exc


def test_symbolic_products_agree_with_the_core_on_the_bundled_profile() -> None:
    registry = KindRegistry.bundled()
    names = registry.kind_names()
    assert names
    ops: tuple[OperationName, ...] = ("mul", "div")
    for a in names:
        for b in names:
            for op in ops:
                if op == "div" and a == b:
                    continue
                left, right = sp.Symbol(a), sp.Symbol(b)
                expr = left * right if op == "mul" else left / right
                symbolic = _symbolic_kind(expr, registry, {a: a, b: b})
                core = _core_kind(registry, a, op, b)
                if isinstance(core, KindError):
                    assert isinstance(symbolic, KindError), f"{a} {op} {b}: {symbolic}"
                else:
                    assert symbolic == core, f"{a} {op} {b}"


def test_the_thermal_heat_capacity_product_agrees() -> None:
    registry = KindRegistry.bundled()
    m, c = sp.Symbol("m"), sp.Symbol("c")
    assert kind_of_expr(m * c, registry=registry, kind_map={"m": "mass", "c": "specific_heat"}) == "heat_capacity"
    assert registry.result_kind("mass", "mul", "specific_heat") == "heat_capacity"

    e, dt = sp.Symbol("E"), sp.Symbol("dT")
    assert (
        kind_of_expr(e / dt, registry=registry, kind_map={"E": "energy", "dT": "temperature_delta"})
        == "heat_capacity"
    )
    assert registry.result_kind("energy", "div", "temperature_delta") == "heat_capacity"


def test_a_mechanics_product_agrees() -> None:
    registry = KindRegistry.bundled()
    force, time = sp.Symbol("F"), sp.Symbol("t")
    assert kind_of_expr(force * time, registry=registry, kind_map={"F": "force", "t": "duration"}) == "momentum"
    assert registry.result_kind("force", "mul", "duration") == "momentum"
    assert registry.result_kind("force", "dot", "displacement") == "energy"
    assert registry.result_kind("displacement", "wedge", "force") == "torque"


def test_kind_rules_have_no_python_copy() -> None:
    package = Path(bridgman.__file__).resolve().parent
    for module in ("kinds.py", "symbolic.py"):
        path = package / module
        assert path.is_file(), path
        tree = ast.parse(path.read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            if isinstance(node, ast.FunctionDef):
                assert node.name not in REMOVED_NAMES, f"{module}: def {node.name}"
            if isinstance(node, ast.Attribute):
                assert node.attr not in REMOVED_NAMES, f"{module}: .{node.attr}"
            if isinstance(node, ast.Name):
                assert node.id not in REMOVED_NAMES, f"{module}: {node.id}"
    assert not hasattr(KindRegistry, "operation_rule")
    assert not hasattr(KindRegistry, "unique_kind_with_dimensions")


def test_operation_names_are_the_cores_product_names() -> None:
    registry = KindRegistry.bundled()
    for name in typing.get_args(OperationName):
        try:
            registry.result_kind("force", name, "displacement")
        except InvalidOperationRuleError as exc:
            pytest.fail(f"{name} is not a core product name: {exc}")
        except KindError:
            pass
    with pytest.raises(InvalidOperationRuleError) as refused:
        registry.result_kind("force", "add", "displacement")  # type: ignore[arg-type]
    assert refused.value.__cause__ is not None
    assert refused.value.__cause__.args == ("invalid_operation", "add")


def test_a_refused_power_keeps_the_native_error() -> None:
    v = sp.Symbol("v")
    with pytest.raises(KindMismatchError) as refused:
        kind_of_expr(v**2, registry=KindRegistry.bundled(), kind_map={"v": "velocity"})
    assert refused.value.__cause__ is not None
    assert refused.value.__cause__.args == ("ungraded_power", "velocity", 2, 1)


def test_a_refused_product_keeps_the_native_error() -> None:
    registry = KindRegistry(
        kinds=[
            QuantityKind("Length", LENGTH),
            QuantityKind("Time", TIME),
            QuantityKind("Force", FORCE),
            QuantityKind("Energy", ENERGY),
            QuantityKind("Torque", ENERGY),
        ],
        rules=[
            OperationRule(
                "Force",
                "mul",
                "Length",
                "Energy",
                commutative=True,
                rationale="Work: W = Fd",
            )
        ],
    )
    force, time = sp.Symbol("F"), sp.Symbol("t")
    with pytest.raises(MissingOperationRuleError) as refused:
        kind_of_expr(force * time, registry=registry, kind_map={"F": "Force", "t": "Time"})
    assert refused.value.__cause__ is not None
    assert refused.value.__cause__.args[0] == "no_product_kind"
    assert str({"M": 1, "L": 1, "T": -1}) in str(refused.value)
