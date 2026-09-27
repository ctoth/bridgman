"""Python asks the Rust core for kind arithmetic; it keeps no rules of its own."""

from __future__ import annotations

import ast
import typing
from fractions import Fraction
from pathlib import Path

import pytest
import sympy as sp

import bridgman
from bridgman import (
    BridgmanError,
    DerivationError,
    KindRegistry,
    OperationParseError,
    OperationRule,
    QuantityError,
    QuantityKind,
    kind_of_expr,
)
from bridgman._core import NativeKindRegistry
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
    "_native_error",
    "_combine_kind_details",
    "_pow_dims_frac",
    "_clean",
    "DIM_ORDER",
}


def _symbolic_kind(expr, registry: KindRegistry, kind_map: dict[str, str]) -> str | BridgmanError:
    try:
        result = kind_of_expr(expr, registry=registry, kind_map=kind_map)
    except BridgmanError as exc:
        return exc
    assert result is not None
    return result


def _registry_kind(registry: KindRegistry, left: str, op: OperationName, right: str) -> str | BridgmanError:
    try:
        return registry.result_kind(left, op, right)
    except BridgmanError as exc:
        return exc


def _native_kind(native: NativeKindRegistry, left: str, op: OperationName, right: str) -> str | BridgmanError:
    """The oracle: the core's product lookup through the binding, bypassing `bridgman.kinds`."""
    try:
        return native.result_kind(left, op, right)
    except BridgmanError as exc:
        return exc


def test_public_products_agree_with_the_native_core_on_the_bundled_catalog() -> None:
    native = NativeKindRegistry.bundled()
    registry = KindRegistry.bundled()
    names = native.kinds()
    assert names
    ops: tuple[OperationName, ...] = ("mul", "div")
    for a in names:
        for b in names:
            for op in ops:
                if op == "div" and a == b:
                    continue
                expected = _native_kind(native, a, op, b)
                left, right = sp.Symbol(a), sp.Symbol(b)
                expr = left * right if op == "mul" else left / right
                observed = {
                    "registry": _registry_kind(registry, a, op, b),
                    "symbolic": _symbolic_kind(expr, registry, {a: a, b: b}),
                }
                for path, result in observed.items():
                    if isinstance(expected, BridgmanError):
                        # sympy writes a*a as a**2, which the core refuses as a power.
                        assert isinstance(result, BridgmanError), f"{path}: {a} {op} {b} -> {result}; core refused {expected}"
                    else:
                        assert result == expected, f"{path}: {a} {op} {b} -> {result}; core says {expected}"


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
    for module in ("kinds.py", "symbolic.py", "dimensions.py"):
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
            if isinstance(node, ast.ClassDef):
                assert not node.name.endswith("Error") or node.name == "UnsupportedExpressionError", (
                    f"{module}: class {node.name} restates a Rust error"
                )
    assert not hasattr(KindRegistry, "operation_rule")
    assert not hasattr(KindRegistry, "unique_kind_with_dimensions")
    assert not hasattr(bridgman, "DimensionalError")


def test_symbolic_judgements_are_the_cores() -> None:
    """Each judgement the walker needs is a Rust operation and its refusal a
    Rust variant: equal terms, transcendental arguments, inexact exponents,
    numbers among quantities, and the kinds of the two sides of an equation."""
    registry = KindRegistry.bundled()
    length, q = sp.Symbol("L"), sp.Symbol("Q")
    kinds = {"L": "length", "Q": "energy"}

    reason = bridgman.explain_expr_kinds(sp.Eq(sp.sin(length), 0), registry=registry, kind_map=kinds).reason
    assert reason.startswith("DimensionError.NotDimensionless: ")
    with pytest.raises(QuantityError.NumberTerm) as number:
        kind_of_expr(q + 1, registry=registry, kind_map=kinds)
    assert number.value.fields == {"operation": "add", "kind": "energy"}
    with pytest.raises(QuantityError.NumberTerm, match="comparison"):
        kind_of_expr(sp.Max(q, 0, evaluate=False), registry=registry, kind_map=kinds)
    with pytest.raises(QuantityError.UnsupportedOperation, match="inexact"):
        kind_of_expr(q ** sp.Symbol("n"), registry=registry, kind_map=kinds)
    assert kind_of_expr(sp.Abs(q), registry=registry, kind_map=kinds) == "energy"
    assert registry.result_kind(None, "mul", "energy") == "energy"
    assert registry.result_kind(None, "add", None) is None
    assert registry.same_kind(None, None) is None
    twins = bridgman.explain_expr_kinds(
        sp.Eq(sp.Symbol("E"), sp.Symbol("tau")),
        registry=registry,
        kind_map={"E": "energy", "tau": "torque"},
    )
    assert twins.reason.startswith("QuantityError.KindMismatch: ")
    assert not bridgman.verify_expr_kinds(sp.Eq(q, 1), registry=registry, kind_map=kinds)
    dims = bridgman.explain_expr(sp.Eq(length, sp.Symbol("t")), {"L": {"L": 1}, "t": {"T": 1}})
    assert dims.reason.startswith("DimensionError.Unequal: ")


def test_operation_names_are_the_cores_operation_names() -> None:
    registry = KindRegistry.bundled()
    for name in typing.get_args(OperationName):
        try:
            registry.result_kind("force", name, "displacement")
        except OperationParseError as exc:
            pytest.fail(f"{name} is not a core operation name: {exc}")
        except BridgmanError:
            pass
    with pytest.raises(OperationParseError.Unknown) as refused:
        registry.result_kind("force", "plus", "displacement")  # type: ignore[arg-type]
    assert refused.value.fields == "plus"
    with pytest.raises(OperationParseError.NotProduct):
        registry.rule_rationale("force", "add", "displacement")  # type: ignore[arg-type]


def test_a_refused_power_is_the_cores_error() -> None:
    v = sp.Symbol("v")
    with pytest.raises(QuantityError.UngradedPower) as refused:
        kind_of_expr(v**2, registry=KindRegistry.bundled(), kind_map={"v": "velocity"})
    assert refused.value.fields == {"base": "velocity", "exponent": "2", "grade": 1}


def test_a_root_of_a_kind_is_the_cores_rational_power() -> None:
    registry = KindRegistry.bundled()
    area = sp.Symbol("A")
    assert kind_of_expr(sp.sqrt(area), registry=registry, kind_map={"A": "area"}) == "length"
    assert registry.power_kind("area", Fraction(1, 2)) == "length"
    with pytest.raises(QuantityError.NoPowerKind) as refused:
        registry.power_kind("length", Fraction(1, 2))
    assert refused.value.fields["dimensions"] == {"L": "1/2"}


def test_a_pure_number_scales_through_the_core() -> None:
    registry = KindRegistry.bundled()
    t, q = sp.Symbol("T"), sp.Symbol("Q")
    assert kind_of_expr(2 * q, registry=registry, kind_map={"Q": "energy"}) == "energy"
    assert kind_of_expr(q / 2, registry=registry, kind_map={"Q": "energy"}) == "energy"
    with pytest.raises(QuantityError.UnsupportedOperation, match="scaling"):
        kind_of_expr(2 * t, registry=registry, kind_map={"T": "temperature"})
    tau = sp.Symbol("tau")
    assert kind_of_expr(1 / tau, registry=registry, kind_map={"tau": "duration"}) == "frequency"


def test_a_point_product_is_the_cores_derivation_error() -> None:
    registry = KindRegistry.bundled()
    with pytest.raises(QuantityError.Derivation) as refused:
        registry.result_kind("thermal_conductance", "mul", "time")
    cause = refused.value.__cause__
    assert isinstance(cause, DerivationError.Point)
    assert cause.fields == {
        "left": "thermal_conductance",
        "op": "mul",
        "right": "time",
        "point": "time",
    }


def test_a_refused_product_is_the_cores_error() -> None:
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
    with pytest.raises(QuantityError.NoProductKind) as refused:
        kind_of_expr(force * time, registry=registry, kind_map={"F": "Force", "t": "Time"})
    assert refused.value.fields["dimensions"] == {"M": "1", "L": "1", "T": "-1"}
    assert "M:1,L:1,T:-1" in str(refused.value)
