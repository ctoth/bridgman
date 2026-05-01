from __future__ import annotations

import sympy as sp

import bridgman


def test_all_public_symbols_resolve() -> None:
    for name in bridgman.__all__:
        assert hasattr(bridgman, name), name


def test_kind_api_is_exported() -> None:
    expected = {
        "QuantityKind",
        "OperationRule",
        "KindRegistry",
        "CheckResult",
        "kind_of_expr",
        "verify_expr_kinds",
        "explain_expr",
        "explain_expr_kinds",
    }

    assert expected <= set(bridgman.__all__)


def test_readme_kind_aware_work_example() -> None:
    energy = {"M": 1, "L": 2, "T": -2}
    force = {"M": 1, "L": 1, "T": -2}
    length = {"L": 1}
    registry = bridgman.KindRegistry(
        kinds=[
            bridgman.QuantityKind("Energy", energy),
            bridgman.QuantityKind("Force", force),
            bridgman.QuantityKind("Length", length),
            bridgman.QuantityKind("Torque", energy),
        ],
        rules=[
            bridgman.OperationRule(
                "Force",
                "mul",
                "Length",
                "Energy",
                commutative=True,
                rationale="Work: W = Fd",
            )
        ],
    )
    energy_symbol, force_symbol, distance_symbol, torque_symbol = sp.symbols("E F d tau")

    assert bridgman.verify_expr_kinds(
        sp.Eq(energy_symbol, force_symbol * distance_symbol),
        registry=registry,
        kind_map={"E": "Energy", "F": "Force", "d": "Length"},
    )
    assert not bridgman.verify_expr_kinds(
        sp.Eq(energy_symbol, torque_symbol),
        registry=registry,
        kind_map={"E": "Energy", "tau": "Torque"},
    )
