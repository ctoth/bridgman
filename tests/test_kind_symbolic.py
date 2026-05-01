from __future__ import annotations

import pytest
import sympy as sp

from bridgman import (
    AmbiguousKindError,
    KindMismatchError,
    KindRegistry,
    MissingOperationRuleError,
    OperationRule,
    QuantityKind,
    UnknownKindError,
    dims_of_expr,
    kind_of_expr,
    verify_expr,
    verify_expr_kinds,
)


LENGTH = {"L": 1}
AREA = {"L": 2}
TIME = {"T": 1}
FORCE = {"M": 1, "L": 1, "T": -2}
ENERGY = {"M": 1, "L": 2, "T": -2}
PRESSURE = {"M": 1, "L": -1, "T": -2}


def mechanics_registry() -> KindRegistry:
    return KindRegistry(
        kinds=[
            QuantityKind("Length", LENGTH),
            QuantityKind("Area", AREA),
            QuantityKind("Time", TIME),
            QuantityKind("Force", FORCE),
            QuantityKind("Energy", ENERGY),
            QuantityKind("Torque", ENERGY),
            QuantityKind("Pressure", PRESSURE),
            QuantityKind("EnergyDensity", PRESSURE),
            QuantityKind("Angle", {}),
            QuantityKind("Unitless", {}),
        ],
        rules=[
            OperationRule("Force", "mul", "Length", "Energy", commutative=True),
            OperationRule("Energy", "div", "Length", "Force"),
        ],
    )


def test_kind_of_expr_uses_declared_operation_rule() -> None:
    force = sp.Symbol("F")
    distance = sp.Symbol("d")

    assert (
        kind_of_expr(
            force * distance,
            registry=mechanics_registry(),
            kind_map={"F": "Force", "d": "Length"},
        )
        == "Energy"
    )


def test_kind_of_expr_uses_commutative_reverse_rule() -> None:
    force = sp.Symbol("F")
    distance = sp.Symbol("d")

    assert (
        kind_of_expr(
            distance * force,
            registry=mechanics_registry(),
            kind_map={"F": "Force", "d": "Length"},
        )
        == "Energy"
    )


def test_verify_expr_kinds_accepts_work_equation() -> None:
    energy = sp.Symbol("E")
    force = sp.Symbol("F")
    distance = sp.Symbol("d")

    assert verify_expr_kinds(
        sp.Eq(energy, force * distance),
        registry=mechanics_registry(),
        kind_map={"E": "Energy", "F": "Force", "d": "Length"},
    )


@pytest.mark.parametrize(
    ("left_kind", "right_kind"),
    (
        ("Energy", "Torque"),
        ("Pressure", "EnergyDensity"),
        ("Angle", "Unitless"),
    ),
)
def test_kind_aware_verification_rejects_dimensional_twins(
    left_kind: str,
    right_kind: str,
) -> None:
    left = sp.Symbol("left")
    right = sp.Symbol("right")

    assert verify_expr(
        sp.Eq(left, right),
        {
            "left": mechanics_registry().kind_dimensions(left_kind),
            "right": mechanics_registry().kind_dimensions(right_kind),
        },
    )
    assert not verify_expr_kinds(
        sp.Eq(left, right),
        registry=mechanics_registry(),
        kind_map={"left": left_kind, "right": right_kind},
    )


def test_kind_addition_requires_identical_kinds() -> None:
    energy = sp.Symbol("E")
    torque = sp.Symbol("tau")

    with pytest.raises(KindMismatchError, match="addition"):
        kind_of_expr(
            energy + torque,
            registry=mechanics_registry(),
            kind_map={"E": "Energy", "tau": "Torque"},
        )


def test_missing_operation_rule_fails_closed() -> None:
    force = sp.Symbol("F")
    time = sp.Symbol("t")

    with pytest.raises(MissingOperationRuleError, match="Force mul Time"):
        kind_of_expr(
            force * time,
            registry=mechanics_registry(),
            kind_map={"F": "Force", "t": "Time"},
        )


def test_unknown_symbol_kind_fails_closed() -> None:
    x = sp.Symbol("x")

    with pytest.raises(UnknownKindError, match="x"):
        kind_of_expr(x, registry=mechanics_registry(), kind_map={})


def test_unknown_registry_kind_fails_closed() -> None:
    x = sp.Symbol("x")

    with pytest.raises(UnknownKindError, match="Velocity"):
        kind_of_expr(x, registry=mechanics_registry(), kind_map={"x": "Velocity"})


def test_integer_power_infers_unique_result_kind_by_dimensions() -> None:
    length = sp.Symbol("d")

    assert (
        kind_of_expr(length**2, registry=mechanics_registry(), kind_map={"d": "Length"})
        == "Area"
    )


def test_power_with_ambiguous_result_dimensions_fails_closed() -> None:
    length = sp.Symbol("d")
    registry = KindRegistry(
        kinds=[
            QuantityKind("Length", LENGTH),
            QuantityKind("Area", AREA),
            QuantityKind("CrossSection", AREA),
        ]
    )

    with pytest.raises(AmbiguousKindError, match="Area"):
        kind_of_expr(length**2, registry=registry, kind_map={"d": "Length"})


def test_kind_aware_acceptance_implies_dimension_only_acceptance() -> None:
    energy = sp.Symbol("E")
    force = sp.Symbol("F")
    distance = sp.Symbol("d")
    equation = sp.Eq(energy, force * distance)
    registry = mechanics_registry()
    kind_map = {"E": "Energy", "F": "Force", "d": "Length"}

    assert verify_expr_kinds(equation, registry=registry, kind_map=kind_map)
    assert verify_expr(
        equation,
        {symbol: registry.kind_dimensions(kind) for symbol, kind in kind_map.items()},
    )
    assert dims_of_expr(force * distance, {"F": FORCE, "d": LENGTH}) == ENERGY
