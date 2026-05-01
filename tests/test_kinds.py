from __future__ import annotations

import pytest

from bridgman import (
    DuplicateKindError,
    DuplicateOperationRuleError,
    InvalidOperationRuleError,
    KindRegistry,
    OperationRule,
    QuantityKind,
    UnknownKindError,
)


MASS = {"M": 1}
LENGTH = {"L": 1}
TIME = {"T": 1}
FORCE = {"M": 1, "L": 1, "T": -2}
ENERGY = {"M": 1, "L": 2, "T": -2}
TORQUE = {"M": 1, "L": 2, "T": -2}


def test_quantity_kind_canonicalizes_dimensions() -> None:
    kind = QuantityKind("Dimensionless", {"M": 0, "L": 0})

    assert kind.dimensions == {}


def test_quantity_kind_rejects_empty_names() -> None:
    with pytest.raises(ValueError, match="non-empty"):
        QuantityKind("", {})


def test_registry_allows_dimensional_twins_with_distinct_names() -> None:
    registry = KindRegistry(
        kinds=[
            QuantityKind("Energy", ENERGY),
            QuantityKind("Torque", TORQUE),
        ]
    )

    assert registry.kind_dimensions("Energy") == ENERGY
    assert registry.kind_dimensions("Torque") == TORQUE
    assert registry.ambiguous_kinds(ENERGY) == ("Energy", "Torque")


def test_registry_rejects_duplicate_kind_names() -> None:
    with pytest.raises(DuplicateKindError, match="Length"):
        KindRegistry(
            kinds=[
                QuantityKind("Length", LENGTH),
                QuantityKind("Length", LENGTH),
            ]
        )


def test_registry_validates_force_length_energy_rule() -> None:
    registry = KindRegistry(
        kinds=[
            QuantityKind("Force", FORCE),
            QuantityKind("Length", LENGTH),
            QuantityKind("Energy", ENERGY),
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

    assert registry.result_kind("Force", "mul", "Length") == "Energy"
    assert registry.result_kind("Length", "mul", "Force") == "Energy"


def test_registry_validates_noncommutative_division_rule() -> None:
    registry = KindRegistry(
        kinds=[
            QuantityKind("Energy", ENERGY),
            QuantityKind("Length", LENGTH),
            QuantityKind("Force", FORCE),
        ],
        rules=[OperationRule("Energy", "div", "Length", "Force")],
    )

    assert registry.result_kind("Energy", "div", "Length") == "Force"
    with pytest.raises(InvalidOperationRuleError, match="No operation rule"):
        registry.result_kind("Length", "div", "Energy")


def test_registry_rejects_duplicate_operation_rules() -> None:
    with pytest.raises(DuplicateOperationRuleError):
        KindRegistry(
            kinds=[
                QuantityKind("Force", FORCE),
                QuantityKind("Length", LENGTH),
                QuantityKind("Energy", ENERGY),
            ],
            rules=[
                OperationRule("Force", "mul", "Length", "Energy", commutative=True),
                OperationRule("Length", "mul", "Force", "Energy", commutative=False),
            ],
        )


def test_registry_rejects_unknown_kind_references() -> None:
    with pytest.raises(UnknownKindError, match="Energy"):
        KindRegistry(
            kinds=[
                QuantityKind("Force", FORCE),
                QuantityKind("Length", LENGTH),
            ],
            rules=[OperationRule("Force", "mul", "Length", "Energy")],
        )


def test_registry_rejects_dimensionally_invalid_rules() -> None:
    with pytest.raises(InvalidOperationRuleError, match="dimensionally invalid"):
        KindRegistry(
            kinds=[
                QuantityKind("Mass", MASS),
                QuantityKind("Length", LENGTH),
                QuantityKind("Time", TIME),
            ],
            rules=[OperationRule("Mass", "mul", "Length", "Time")],
        )


def test_kinds_with_dimensions_returns_exact_canonical_matches() -> None:
    registry = KindRegistry(
        kinds=[
            QuantityKind("Energy", ENERGY),
            QuantityKind("Torque", TORQUE),
            QuantityKind("Length", LENGTH),
        ]
    )

    assert registry.kinds_with_dimensions({"T": -2, "M": 1, "L": 2}) == (
        "Energy",
        "Torque",
    )
    assert registry.kinds_with_dimensions({"L": 1}) == ("Length",)
    assert registry.ambiguous_kinds({"L": 1}) == ()
