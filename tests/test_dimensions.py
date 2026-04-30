from bridgman import (
    mul_dims,
    div_dims,
    pow_dims,
    dims_equal,
    is_dimensionless,
    format_dims,
)


# --- mul_dims ---

def test_mul_dims_identity():
    assert mul_dims({}, {"M": 1}) == {"M": 1}
    assert mul_dims({"M": 1}, {}) == {"M": 1}
    assert mul_dims({}, {}) == {}


def test_mul_dims_single():
    assert mul_dims({"M": 1}, {"L": 1}) == {"M": 1, "L": 1}


def test_mul_dims_mixed():
    # mass * acceleration = force
    assert mul_dims({"M": 1}, {"L": 1, "T": -2}) == {"M": 1, "L": 1, "T": -2}


def test_mul_dims_cancellation():
    # velocity * time = length (T cancels)
    assert mul_dims({"L": 1, "T": -1}, {"T": 1}) == {"L": 1}


# --- div_dims ---

def test_div_dims_same_cancels():
    assert div_dims({"L": 1}, {"L": 1}) == {}


def test_div_dims_mixed():
    # length / time = velocity
    assert div_dims({"L": 1}, {"T": 1}) == {"L": 1, "T": -1}


# --- pow_dims ---

def test_pow_dims_square():
    assert pow_dims({"L": 1}, 2) == {"L": 2}


def test_pow_dims_cube():
    assert pow_dims({"L": 1}, 3) == {"L": 3}


def test_pow_dims_zero():
    assert pow_dims({"L": 1, "T": -2}, 0) == {}


# --- dims_equal ---

def test_dims_equal_missing_keys_as_zero():
    assert dims_equal({"L": 1, "T": 0}, {"L": 1})


def test_dims_equal_zero_equals_empty():
    assert dims_equal({"L": 0}, {})


def test_dims_equal_not_equal():
    assert not dims_equal({"L": 1}, {"L": 2})


# --- is_dimensionless ---

def test_is_dimensionless_empty():
    assert is_dimensionless({})


def test_is_dimensionless_all_zero():
    assert is_dimensionless({"L": 0, "T": 0})


def test_is_dimensionless_non_empty():
    assert not is_dimensionless({"L": 1})


# --- format_dims ---

def test_format_dims_force():
    result = format_dims({"M": 1, "L": 1, "T": -2})
    assert "M" in result
    assert "L" in result
    assert "T" in result
    # negative exponent should use superscript
    assert "\u207b\u00b2" in result  # ⁻²


def test_format_dims_ordering():
    # Should have consistent ordering
    r1 = format_dims({"M": 1, "L": 1, "T": -2})
    r2 = format_dims({"T": -2, "L": 1, "M": 1})
    assert r1 == r2


def test_format_dims_dimensionless():
    assert format_dims({}) == "1"
