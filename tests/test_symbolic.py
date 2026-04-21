"""Tests for sympy expression dimensional analysis."""

import pytest
from sympy import Symbol, sqrt, Eq, Rational, pi

from bridgman.symbolic import dims_of_expr, verify_expr, DimensionalError


# Common dimension maps for physics
@pytest.fixture
def dim_map():
    m = Symbol("m")
    a = Symbol("a")
    F = Symbol("F")
    v = Symbol("v")
    E = Symbol("E")
    c = Symbol("c")
    KE = Symbol("KE")
    t = Symbol("t")
    r = Symbol("r")
    G = Symbol("G")
    m1 = Symbol("m1")
    m2 = Symbol("m2")
    eps0 = Symbol("eps0")
    mu0 = Symbol("mu0")
    F_grav = Symbol("F_grav")

    return {
        "m": {"M": 1},
        "m1": {"M": 1},
        "m2": {"M": 1},
        "a": {"L": 1, "T": -2},
        "F": {"M": 1, "L": 1, "T": -2},
        "F_grav": {"M": 1, "L": 1, "T": -2},
        "v": {"L": 1, "T": -1},
        "E": {"M": 1, "L": 2, "T": -2},
        "c": {"L": 1, "T": -1},
        "KE": {"M": 1, "L": 2, "T": -2},
        "t": {"T": 1},
        "r": {"L": 1},
        "G": {"M": -1, "L": 3, "T": -2},
        "eps0": {"M": -1, "L": -3, "T": 4, "I": 2},
        "mu0": {"M": 1, "L": 1, "T": -2, "I": -2},
    }


def test_dims_of_symbol(dim_map):
    """Simple symbol lookup returns its dimensions."""
    m = Symbol("m")
    assert dims_of_expr(m, dim_map) == {"M": 1}


def test_dims_of_mul(dim_map):
    """F = m * a -> {M:1, L:1, T:-2}."""
    m, a = Symbol("m"), Symbol("a")
    result = dims_of_expr(m * a, dim_map)
    assert result == {"M": 1, "L": 1, "T": -2}


def test_dims_of_pow(dim_map):
    """v**2 -> {L:2, T:-2}."""
    v = Symbol("v")
    result = dims_of_expr(v**2, dim_map)
    assert result == {"L": 2, "T": -2}


def test_symbolic_pow_wraps_type_error(dim_map):
    """x**n with symbolic n raises a dimensional error, not a raw TypeError."""
    x, n = Symbol("x"), Symbol("n")
    with pytest.raises(DimensionalError, match="non-numeric exponent"):
        dims_of_expr(x**n, {**dim_map, "x": {"L": 1}})


def test_dims_of_div(dim_map):
    """v / t -> {L:1, T:-2} (acceleration)."""
    v, t = Symbol("v"), Symbol("t")
    result = dims_of_expr(v / t, dim_map)
    assert result == {"L": 1, "T": -2}


def test_dims_of_numeric_constant(dim_map):
    """0.5 * m * v**2 -> energy dims {M:1, L:2, T:-2}."""
    m, v = Symbol("m"), Symbol("v")
    result = dims_of_expr(Rational(1, 2) * m * v**2, dim_map)
    assert result == {"M": 1, "L": 2, "T": -2}


def test_dims_of_sqrt(dim_map):
    """sqrt(eps0 * mu0) should have dims {L:-1, T:1}.

    eps0: {M:-1, L:-3, T:4, I:2}
    mu0:  {M:1,  L:1,  T:-2, I:-2}
    product: {L:-2, T:2}
    sqrt:    {L:-1, T:1}
    """
    eps0, mu0 = Symbol("eps0"), Symbol("mu0")
    result = dims_of_expr(sqrt(eps0 * mu0), dim_map)
    assert result == {"L": -1, "T": 1}


def test_dims_of_add_matching(dim_map):
    """m*a + m*a should work (same dims: force)."""
    m, a = Symbol("m"), Symbol("a")
    result = dims_of_expr(m * a + m * a, dim_map)
    assert result == {"M": 1, "L": 1, "T": -2}


def test_dims_of_add_mismatch(dim_map):
    """m + v should raise DimensionalError."""
    m, v = Symbol("m"), Symbol("v")
    with pytest.raises(DimensionalError):
        dims_of_expr(m + v, dim_map)


def test_verify_expr_fma(dim_map):
    """Eq(F, m*a) -> True."""
    F, m, a = Symbol("F"), Symbol("m"), Symbol("a")
    assert verify_expr(Eq(F, m * a), dim_map) is True


def test_verify_expr_emc2(dim_map):
    """Eq(E, m*c**2) -> True."""
    E, m, c = Symbol("E"), Symbol("m"), Symbol("c")
    assert verify_expr(Eq(E, m * c**2), dim_map) is True


def test_verify_expr_ke(dim_map):
    """Eq(KE, 0.5*m*v**2) -> True."""
    KE, m, v = Symbol("KE"), Symbol("m"), Symbol("v")
    assert verify_expr(Eq(KE, Rational(1, 2) * m * v**2), dim_map) is True


def test_verify_expr_wrong(dim_map):
    """Eq(F, m*v) -> False (momentum != force)."""
    F, m, v = Symbol("F"), Symbol("m"), Symbol("v")
    assert verify_expr(Eq(F, m * v), dim_map) is False


def test_verify_expr_speed_of_light(dim_map):
    """Eq(c, 1/sqrt(eps0*mu0)) -> True.

    1/sqrt(eps0*mu0) has dims {L:1, T:-1} = velocity.
    """
    c, eps0, mu0 = Symbol("c"), Symbol("eps0"), Symbol("mu0")
    assert verify_expr(Eq(c, 1 / sqrt(eps0 * mu0)), dim_map) is True


def test_verify_expr_gravitational_force(dim_map):
    """Eq(F_grav, G*m1*m2/r**2) -> True.

    G: {M:-1, L:3, T:-2}
    m1*m2: {M:2}
    r**2: {L:2}
    G*m1*m2/r**2: {M:1, L:1, T:-2} = force
    """
    F_grav = Symbol("F_grav")
    G, m1, m2, r = Symbol("G"), Symbol("m1"), Symbol("m2"), Symbol("r")
    assert verify_expr(Eq(F_grav, G * m1 * m2 / r**2), dim_map) is True


def test_missing_symbol_raises(dim_map):
    """Unknown symbol raises KeyError."""
    x = Symbol("x")
    with pytest.raises(KeyError):
        dims_of_expr(x, dim_map)
