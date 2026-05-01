from __future__ import annotations

import sympy as sp

from bridgman import (
    KindRegistry,
    OperationRule,
    QuantityKind,
    explain_expr_kinds,
    verify_expr,
    verify_expr_kinds,
)


ENERGY = {"M": 1, "L": 2, "T": -2}
FORCE = {"M": 1, "L": 1, "T": -2}
LENGTH = {"L": 1}
TIME = {"T": 1}

ENERGY_KIND = "ps:concept:energy"
TORQUE_KIND = "ps:concept:torque"
FORCE_KIND = "ps:concept:force"
LENGTH_KIND = "ps:concept:length"
ARTIFACT_KIND = "ps:artifact:energy-form-v1"
TIME_KIND = "ps:form:time_form"
ANGLE_KIND = "ps:concept:angle"


def propstore_registry(*, result_kind: str = ENERGY_KIND) -> KindRegistry:
    return KindRegistry(
        kinds=[
            QuantityKind(ENERGY_KIND, ENERGY),
            QuantityKind(TORQUE_KIND, ENERGY),
            QuantityKind(ARTIFACT_KIND, ENERGY),
            QuantityKind(FORCE_KIND, FORCE),
            QuantityKind(LENGTH_KIND, LENGTH),
            QuantityKind(TIME_KIND, TIME),
            QuantityKind(ANGLE_KIND, {}),
        ],
        rules=[
            OperationRule(
                FORCE_KIND,
                "mul",
                LENGTH_KIND,
                result_kind,
                commutative=True,
                rationale="Propstore form algebra: work = force * displacement",
            )
        ],
    )


def test_propstore_concept_ids_work_as_kind_names() -> None:
    registry = propstore_registry()

    assert registry.kind_dimensions(ENERGY_KIND) == ENERGY
    assert registry.kind_dimensions("ps:concept:torque") == ENERGY
    assert registry.kind_dimensions(ARTIFACT_KIND) == ENERGY


def test_propstore_symbol_bindings_validate_work_equation() -> None:
    energy = sp.Symbol("E")
    force = sp.Symbol("F")
    displacement = sp.Symbol("d")

    assert verify_expr_kinds(
        sp.Eq(energy, force * displacement),
        registry=propstore_registry(),
        kind_map={"E": ENERGY_KIND, "F": FORCE_KIND, "d": LENGTH_KIND},
    )


def test_propstore_symbol_bindings_can_target_torque_when_rule_says_torque() -> None:
    torque = sp.Symbol("tau")
    force = sp.Symbol("F")
    displacement = sp.Symbol("d")

    assert verify_expr_kinds(
        sp.Eq(torque, force * displacement),
        registry=propstore_registry(result_kind=TORQUE_KIND),
        kind_map={"tau": TORQUE_KIND, "F": FORCE_KIND, "d": LENGTH_KIND},
    )
    assert not verify_expr_kinds(
        sp.Eq(torque, force * displacement),
        registry=propstore_registry(result_kind=ENERGY_KIND),
        kind_map={"tau": TORQUE_KIND, "F": FORCE_KIND, "d": LENGTH_KIND},
    )


def test_propstore_energy_torque_is_dimension_valid_but_kind_invalid() -> None:
    energy = sp.Symbol("E")
    torque = sp.Symbol("tau")

    assert verify_expr(sp.Eq(energy, torque), {"E": ENERGY, "tau": ENERGY})
    assert not verify_expr_kinds(
        sp.Eq(energy, torque),
        registry=propstore_registry(),
        kind_map={"E": ENERGY_KIND, "tau": TORQUE_KIND},
    )


def test_propstore_sin_length_reports_structured_error() -> None:
    angle = sp.Symbol("angle")
    length = sp.Symbol("length")
    result = explain_expr_kinds(
        sp.Eq(angle, sp.sin(length)),
        registry=propstore_registry(),
        kind_map={"angle": ANGLE_KIND, "length": LENGTH_KIND},
    )

    assert not result.ok
    assert "sin argument must be dimensionless" in result.reason
    assert result.lhs_kind == ANGLE_KIND


def test_propstore_missing_operation_rule_reports_operation_and_result_dimensions() -> None:
    energy = sp.Symbol("E")
    force = sp.Symbol("F")
    time = sp.Symbol("t")
    result = explain_expr_kinds(
        sp.Eq(energy, force * time),
        registry=propstore_registry(),
        kind_map={"E": ENERGY_KIND, "F": FORCE_KIND, "t": TIME_KIND},
    )

    assert not result.ok
    assert FORCE_KIND in result.reason
    assert "mul" in result.reason
    assert TIME_KIND in result.reason
    assert str({"M": 1, "L": 1, "T": -1}) in result.reason


def test_propstore_successful_rule_reports_rationale() -> None:
    energy = sp.Symbol("E")
    force = sp.Symbol("F")
    displacement = sp.Symbol("d")
    result = explain_expr_kinds(
        sp.Eq(energy, force * displacement),
        registry=propstore_registry(),
        kind_map={"E": ENERGY_KIND, "F": FORCE_KIND, "d": LENGTH_KIND},
    )

    assert result.ok
    assert any("Propstore form algebra" in step for step in result.steps)
