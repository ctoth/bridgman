# bridgman

Dimensional analysis arithmetic for SI quantities. Named after
[Percy Bridgman](https://en.wikipedia.org/wiki/Percy_Williams_Bridgman).

Bridgman works with dimension dictionaries whose keys are SI base-dimension
symbols and whose values are integer exponents:

```python
force = {"M": 1, "L": 1, "T": -2}
```

## Install

```powershell
uv add bridgman
```

Install the symbolic API with SymPy support:

```powershell
uv add "bridgman[sympy]"
```

## Dict API

- `Dimensions`: `dict[str, int]` type alias.
- `mul_dims(d1, d2)`: multiply quantities by adding exponents.
- `div_dims(d1, d2)`: divide quantities by subtracting exponents.
- `pow_dims(d, n)`: raise dimensions to an integer power. `n` must be an
  `int`; `bool` and non-integer exponents raise `TypeError`.
- `dims_equal(d1, d2)`: compare after removing zero exponents.
- `is_dimensionless(d)`: return true when all exponents are zero or absent.
- `format_dims(d)`: produce display text such as `M L T^-2`.
- `dims_signature(d)`: produce a canonical, zero-stripped signature such as
  `M:1,L:1,T:-2`, with dimensionless values represented as `1`.
- `parse_dims_signature(signature)`: parse a signature produced by
  `dims_signature`.
- `canonicalize_dims(d)`: normalize dimension keys. `Theta`, uppercase theta,
  and lowercase theta all canonicalize to `Theta`.

## Symbolic API

The symbolic API requires SymPy. Its canonical checking entry point is
`verify_expr`.

- `dims_of_expr(expr, dim_map)`: compute dimensions for a SymPy expression.
- `verify_expr(eq, dim_map)`: verify a SymPy `Eq` by comparing the dimensions
  of both sides.
- `DimensionalError`: raised for proven dimensional inconsistency or unsupported
  symbolic constructs.
- `SympyRequiredError`: raised by symbolic APIs when SymPy is not installed.

Supported expression forms are symbols, numbers, multiplication, powers,
addition, and equality through `verify_expr`. Addition requires every term to
share dimensions. Powers require exact integer or rational exponents; SymPy
`Float` exponents are rejected because they are not exact dimensional claims.

The following functions require dimensionless arguments and return
dimensionless results: `sin`, `cos`, `tan`, `exp`, `log`, `sinh`, `cosh`,
`tanh`, and `atan2`. If any argument has dimensions, Bridgman raises
`DimensionalError`.

Unsupported SymPy nodes, including derivatives, integrals, piecewise
expressions, min/max, absolute value, Kronecker deltas, and nested equalities,
raise `DimensionalError` rather than being silently accepted.

## Example

```python
import sympy as sp
from bridgman import verify_expr

F, m, a = sp.symbols("F m a")

dim_map = {
    "F": {"M": 1, "L": 1, "T": -2},
    "m": {"M": 1},
    "a": {"L": 1, "T": -2},
}

assert verify_expr(sp.Eq(F, m * a), dim_map)
```

## License

MIT
