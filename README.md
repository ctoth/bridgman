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
- `format_dims(d)`: produce display text such as `M L T⁻²`, using Unicode
  superscripts. Dimensionless values render as `1`.
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
- `verify_expr(eq, dim_map)`: verify a SymPy equality or inequality by
  comparing the dimensions of both sides.
- `explain_expr(eq, dim_map)`: return a structured dimension-only
  `CheckResult` exposing `ok`, `lhs_dimensions`, `rhs_dimensions`, `reason`,
  and `steps`.
- `DimensionalError`: raised for proven dimensional inconsistency or unsupported
  symbolic constructs.
- `SympyRequiredError`: raised by symbolic APIs when SymPy is not installed.

Supported expression forms are symbols, numbers, multiplication, powers,
addition, `Abs`, `Min`, `Max`, and equality or inequality through `verify_expr`.
Addition, `Min`, and `Max` require every term to share dimensions. `Abs`
preserves the argument dimensions. Powers of dimensioned quantities require
exact integer or rational exponents; SymPy `Float` exponents are rejected
because they are not exact dimensional claims. Dimensionless bases may be raised
to arbitrary symbolic or floating exponents because the result remains
dimensionless.

The following functions require dimensionless arguments and return
dimensionless results: `sin`, `cos`, `tan`, `exp`, `log`, `sinh`, `cosh`,
and `tanh`. If any argument has dimensions, Bridgman raises
`DimensionalError`. `atan2(y, x)` is different: it requires `y` and `x` to have
equal dimensions, and returns a dimensionless result.

Unsupported SymPy nodes, including derivatives, integrals, piecewise
expressions, Kronecker deltas, and nested relational expressions, raise
`DimensionalError` rather than being silently accepted.

## Kind API

Dimensions say whether an equation is dimensionally possible. They do not say
whether two dimensionally identical quantities mean the same thing. Energy and
torque both have dimensions `{"M": 1, "L": 2, "T": -2}`; pressure and energy
density also collide; angle and plain unitless values are both dimensionless.

The semantic kind layer keeps the dimension dictionaries as the arithmetic core
and adds named quantity kinds above them:

```python
import sympy as sp
from bridgman import KindRegistry, OperationRule, QuantityKind, verify_expr_kinds

E, F, d, tau = sp.symbols("E F d tau")

energy = {"M": 1, "L": 2, "T": -2}
force = {"M": 1, "L": 1, "T": -2}
length = {"L": 1}

registry = KindRegistry(
    kinds=[
        QuantityKind("Energy", energy),
        QuantityKind("Torque", energy),
        QuantityKind("Force", force),
        QuantityKind("Length", length),
    ],
    rules=[
        OperationRule(
            "Force",
            "mul",
            "Length",
            "Energy",
            commutative=True,
            rationale="Work: W = Fd",
        ),
    ],
)

assert verify_expr_kinds(
    sp.Eq(E, F * d),
    registry=registry,
    kind_map={"E": "Energy", "F": "Force", "d": "Length"},
)

assert not verify_expr_kinds(
    sp.Eq(E, tau),
    registry=registry,
    kind_map={"E": "Energy", "tau": "Torque"},
)
```

- `QuantityKind(name, dimensions)`: declares a semantic kind. Names are
  arbitrary non-empty strings and do not need to be valid Python identifiers.
  Dimensions are canonicalized at construction.
- `OperationRule(left_kind, op, right_kind, result_kind, commutative=False,
  rationale=None)`: declares a semantic `"mul"` or `"div"` rule. Set
  `commutative=True` to register both argument orders for multiplication
  (division rules cannot be commutative). `rationale` is an optional human
  string surfaced in `CheckResult.steps`.
- `KindRegistry(kinds=[...], rules=[...])`: validates kind definitions and
  operation rules. Operation rules are checked for dimensional consistency at
  registration; dimensionally invalid rules raise `InvalidOperationRuleError`.
  Introspection methods: `kind_dimensions(name)`, `result_kind(left, op,
  right)`, `operation_rule(left, op, right)`, `kinds_with_dimensions(d)`,
  `unique_kind_with_dimensions(d)`, and `ambiguous_kinds(d)`.
- `kind_of_expr(expr, registry=..., kind_map=...)`: infers the semantic kind of
  a SymPy expression.
- `verify_expr_kinds(eq, registry=..., kind_map=...)`: verifies both dimensions
  and semantic kind.
- `explain_expr_kinds(eq, registry=..., kind_map=...)`: returns a structured
  `CheckResult` with kinds, dimensions, reason text, and operation steps.

### Kind errors

All kind errors derive from `KindError`:

- `DuplicateKindError`: same kind name registered twice.
- `DuplicateOperationRuleError`: same `(left, op, right)` registered twice
  (including the implicit reverse direction of a commutative rule).
- `UnknownKindError`: a referenced kind name is not in the registry, or no
  registered kind has the supplied dimensions.
- `InvalidOperationRuleError`: an operation rule uses an unsupported `op` or
  is dimensionally inconsistent with its declared result.
- `MissingOperationRuleError` (subclass of `InvalidOperationRuleError`): no
  rule exists for a requested `(left, op, right)` triple encountered during
  expression evaluation.
- `AmbiguousKindError`: dimensions match more than one registered kind when a
  unique kind is required.
- `KindMismatchError`: incompatible kinds combined in an operation that
  requires equal kinds (e.g. addition, `Min`, `Max`).

Kind-aware checking is stricter than dimension-only checking. Every
kind-accepted equation should also be dimensionally accepted, but dimensionally
accepted equations over semantic twins can still be rejected by kind-aware
verification.

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
