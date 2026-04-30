from __future__ import annotations

from bridgman import dims_signature, parse_dims_signature


def test_dims_signature_is_order_insensitive() -> None:
    assert dims_signature({"T": -2, "M": 1, "L": 1}) == dims_signature(
        {"L": 1, "M": 1, "T": -2}
    )


def test_dims_signature_omits_zero_exponents() -> None:
    assert dims_signature({"L": 1, "T": 0}) == "L:1"


def test_parse_dims_signature_round_trips_signature() -> None:
    dims = {"M": 1, "L": 2, "T": -2, "Theta": 1}
    signature = dims_signature(dims)

    assert parse_dims_signature(signature) == dims
    assert dims_signature(parse_dims_signature(signature)) == signature


def test_dimensionless_signature_round_trips() -> None:
    assert dims_signature({}) == "1"
    assert parse_dims_signature("1") == {}
