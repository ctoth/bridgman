from __future__ import annotations

import sympy as sp

from bridgman import (
    CheckResult,
    KindRegistry,
    OperationRule,
    QuantityKind,
    explain_expr,
    explain_expr_kinds,
    verify_expr,
    verify_expr_kinds,
)


LENGTH = {"L": 1}
TIME = {"T": 1}
FORCE = {"M": 1, "L": 1, "T": -2}
ENERGY = {"M": 1, "L": 2, "T": -2}


def mechanics_registry() -> KindRegistry:
    return KindRegistry(
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


def test_explain_expr_reports_dimension_mismatch() -> None:
    force = sp.Symbol("F")
    length = sp.Symbol("d")
    result = explain_expr(sp.Eq(force, length), {"F": FORCE, "d": LENGTH})

    assert isinstance(result, CheckResult)
    assert not result.ok
    assert result.lhs_dimensions == FORCE
    assert result.rhs_dimensions == LENGTH
    assert "dimension mismatch" in result.reason
    assert result.ok == verify_expr(sp.Eq(force, length), {"F": FORCE, "d": LENGTH})


def test_explain_expr_kinds_reports_kind_mismatch_for_dimensional_twins() -> None:
    energy = sp.Symbol("E")
    torque = sp.Symbol("tau")
    result = explain_expr_kinds(
        sp.Eq(energy, torque),
        registry=mechanics_registry(),
        kind_map={"E": "Energy", "tau": "Torque"},
    )

    assert not result.ok
    assert result.lhs_kind == "Energy"
    assert result.rhs_kind == "Torque"
    assert result.lhs_dimensions == ENERGY
    assert result.rhs_dimensions == ENERGY
    assert "kind mismatch" in result.reason
    assert result.ok == verify_expr_kinds(
        sp.Eq(energy, torque),
        registry=mechanics_registry(),
        kind_map={"E": "Energy", "tau": "Torque"},
    )


def test_explain_expr_kinds_reports_missing_operation_rule_details() -> None:
    force = sp.Symbol("F")
    time = sp.Symbol("t")
    energy = sp.Symbol("E")
    result = explain_expr_kinds(
        sp.Eq(energy, force * time),
        registry=mechanics_registry(),
        kind_map={"E": "Energy", "F": "Force", "t": "Time"},
    )

    assert not result.ok
    assert "missing operation rule" in result.reason
    assert "Force mul Time" in result.reason
    assert any("Force mul Time" in step for step in result.steps)


def test_explain_expr_kinds_includes_successful_rule_rationale() -> None:
    energy = sp.Symbol("E")
    force = sp.Symbol("F")
    length = sp.Symbol("d")
    result = explain_expr_kinds(
        sp.Eq(energy, force * length),
        registry=mechanics_registry(),
        kind_map={"E": "Energy", "F": "Force", "d": "Length"},
    )

    assert result.ok
    assert result.lhs_kind == "Energy"
    assert result.rhs_kind == "Energy"
    assert "same kind" in result.reason
    assert any("Work: W = Fd" in step for step in result.steps)
