from __future__ import annotations

from bridgman import canonicalize_dims


def test_canonicalize_dims_maps_theta_glyphs_to_theta_name() -> None:
    upper_theta = chr(0x0398)
    lower_theta = chr(0x03B8)

    assert canonicalize_dims({upper_theta: 1}) == {"Theta": 1}
    assert canonicalize_dims({lower_theta: 1}) == {"Theta": 1}
    assert canonicalize_dims({"Theta": 1}) == {"Theta": 1}


def test_canonicalize_dims_combines_equivalent_keys() -> None:
    upper_theta = chr(0x0398)

    assert canonicalize_dims({upper_theta: 1, "Theta": 2}) == {"Theta": 3}


def test_canonicalize_dims_drops_zero_exponents() -> None:
    assert canonicalize_dims({"L": 1, "T": 0}) == {"L": 1}
