"""Every Rust error variant is raised as its own Python class, generated from
the Rust enums; nothing falls back to ValueError."""

from __future__ import annotations

from fractions import Fraction

import pytest

import bridgman
from bridgman import (
    BridgmanError,
    CatalogError,
    DerivationError,
    DimensionError,
    KindRegistry,
    OperationParseError,
    OperationRule,
    QuantityError,
    QuantityKind,
    parse_dims_signature,
)

FAMILIES = (CatalogError, QuantityError, DerivationError, DimensionError, OperationParseError)


def test_each_rust_variant_has_its_own_class() -> None:
    assert not issubclass(BridgmanError, ValueError)
    seen: set[type] = set()
    for family in FAMILIES:
        assert issubclass(family, BridgmanError)
        assert family.variants, family
        for name in family.variants:
            variant = getattr(family, name)
            assert issubclass(variant, family), (family, name)
            assert variant.__qualname__ == f"{family.__qualname__}.{name}"
            assert variant.__module__ == "bridgman"
            assert variant not in seen
            seen.add(variant)
    assert "KindMismatch" in QuantityError.variants
    assert "Derivation" in CatalogError.variants and "Derivation" in QuantityError.variants
    assert set(DerivationError.variants) == {"Unknown", "UnresolvedDimensions", "Point", "Ungraded"}


def test_refusals_raise_their_variant_with_its_fields() -> None:
    registry = KindRegistry.bundled()
    with pytest.raises(QuantityError.KindMismatch) as mismatch:
        registry.same_kind("energy", "torque")
    assert type(mismatch.value) is QuantityError.KindMismatch
    assert mismatch.value.fields == {"expected": "energy", "actual": "torque"}

    with pytest.raises(DimensionError.InvalidPower) as power:
        parse_dims_signature("L:1/0")
    assert power.value.fields["text"] == "1/0"

    with pytest.raises(OperationParseError.Unknown):
        registry.result_kind("force", "times", "displacement")  # type: ignore[arg-type]

    with pytest.raises(CatalogError.Duplicate) as duplicate:
        KindRegistry(kinds=[QuantityKind("L", {"L": 1}), QuantityKind("L", {"L": 1})])
    assert duplicate.value.fields == {"record": "kind", "id": "L"}


def test_a_wrapped_derivation_error_is_the_cause() -> None:
    with pytest.raises(CatalogError.Derivation) as refused:
        KindRegistry(
            kinds=[QuantityKind("x", {"L": 1})],
            rules=[OperationRule("x", "mul", "x", "missing")],
        )
    assert type(refused.value.__cause__) is DerivationError.Unknown
    assert refused.value.__cause__.fields == {"record": "kind", "id": "missing"}


def test_no_python_module_restates_a_rust_error() -> None:
    for name in dir(bridgman):
        value = getattr(bridgman, name)
        if isinstance(value, type) and issubclass(value, Exception):
            assert (
                issubclass(value, BridgmanError)
                or name in {"UnsupportedExpressionError", "PiError", "SympyRequiredError"}
            ), name


def test_rational_exponents_cross_the_boundary_exactly() -> None:
    assert bridgman.pow_dims({"L": 1}, Fraction(1, 2)) == {"L": Fraction(1, 2)}
    assert bridgman.pow_dims({"L": Fraction(1, 2)}, 2) == {"L": 1}
    with pytest.raises(TypeError):
        bridgman.pow_dims({"L": 1}, 0.5)  # type: ignore[arg-type]
