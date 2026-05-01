from __future__ import annotations

import pytest
import sympy as sp

from bridgman import verify_expr


TWIN_DIMENSIONS = {
    "Energy": {"M": 1, "L": 2, "T": -2},
    "Torque": {"M": 1, "L": 2, "T": -2},
    "Pressure": {"M": 1, "L": -1, "T": -2},
    "EnergyDensity": {"M": 1, "L": -1, "T": -2},
    "SpecificEnergy": {"L": 2, "T": -2},
    "AbsorbedDose": {"L": 2, "T": -2},
    "Frequency": {"T": -1},
    "Activity": {"T": -1},
    "Angle": {},
    "Unitless": {},
}


@pytest.mark.parametrize(
    ("left_kind", "right_kind"),
    (
        ("Energy", "Torque"),
        ("Pressure", "EnergyDensity"),
        ("SpecificEnergy", "AbsorbedDose"),
        ("Frequency", "Activity"),
        ("Angle", "Unitless"),
    ),
)
def test_dimension_only_verification_cannot_distinguish_dimensional_twins(
    left_kind: str,
    right_kind: str,
) -> None:
    left = sp.Symbol("left")
    right = sp.Symbol("right")

    assert verify_expr(
        sp.Eq(left, right),
        {"left": TWIN_DIMENSIONS[left_kind], "right": TWIN_DIMENSIONS[right_kind]},
    )


@pytest.mark.xfail(
    strict=True,
    raises=(AttributeError, ImportError),
    reason="Kind-aware symbolic verification lands after the registry API.",
)
def test_kind_aware_verification_target_rejects_energy_torque_equivalence() -> None:
    import bridgman

    energy = sp.Symbol("E")
    torque = sp.Symbol("tau")
    registry = bridgman.KindRegistry(
        kinds=[
            bridgman.QuantityKind("Energy", TWIN_DIMENSIONS["Energy"]),
            bridgman.QuantityKind("Torque", TWIN_DIMENSIONS["Torque"]),
        ]
    )

    assert not bridgman.verify_expr_kinds(
        sp.Eq(energy, torque),
        registry=registry,
        kind_map={"E": "Energy", "tau": "Torque"},
    )
