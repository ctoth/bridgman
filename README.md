# bridgman

Dimensional analysis arithmetic for SI quantities. Named after
[Percy Bridgman](https://en.wikipedia.org/wiki/Percy_Williams_Bridgman).

Bridgman works with dimension dictionaries whose keys are base-dimension
identifiers and whose values are exact exponents:

```python
from fractions import Fraction

force = {"M": 1, "L": 1, "T": -2}
root_length = {"L": Fraction(1, 2)}
```

The bases are open. The seven SI bases (`M`, `L`, `T`, `I`, `Theta`, `N`,
`J`) come first in signatures and displays, and any other identifier (say
`"user:money"`) is a base of its own, ordered after them. Exponents are exact
rationals: an `int`, or a `fractions.Fraction` when a root makes one
fractional. All arithmetic, including the order of bases, is the Rust core's;
the Python package only converts to and from it.

## Install

```powershell
uv add bridgman
```

Install the symbolic API with SymPy support:

```powershell
uv add "bridgman[sympy]"
```

## Dict API

- `Dimensions`: `dict[str, int | Fraction]` type alias.
- `mul_dims(d1, d2)`: multiply quantities by adding exponents.
- `div_dims(d1, d2)`: divide quantities by subtracting exponents.
- `pow_dims(d, n)`: raise dimensions to an exact power. `n` is an `int` or a
  `Fraction` (`Fraction(1, 2)` is a square root); `bool`, `float` and other
  values raise `TypeError`.
- `dims_equal(d1, d2)`: compare after removing zero exponents.
- `is_dimensionless(d)`: return true when all exponents are zero or absent.
- `format_dims(d)`: produce display text such as `M L T⁻²`, using Unicode
  superscripts, in signature order. Dimensionless values render as `1`.
- `dims_signature(d)`: produce a canonical, zero-stripped signature such as
  `M:1,L:1,T:-2` or `L:1/2`, with dimensionless values represented as `1`.
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
- `UnsupportedExpressionError` (a `TypeError`): raised for a SymPy construct the
  walker cannot read. Every dimensional judgement is the Rust core's, raised as
  its own variant (see Errors below).
- `SympyRequiredError`: raised by symbolic APIs when SymPy is not installed.

The symbolic layer only walks SymPy trees; each rule it applies is a Rust
operation. Supported expression forms are symbols, numbers, multiplication,
powers, addition, `Abs`, `Min`, `Max`, and equality or inequality through
`verify_expr`. Addition, `Min`, and `Max` need terms of equal dimensions
(`common_dims`, refused as `DimensionError.Unequal`). `Abs` preserves the
argument dimensions. Powers take exact integer or rational exponents
(`pow_dims`): `sqrt(length)` has dimensions `{"L": Fraction(1, 2)}`, as in
Rust. A symbolic or floating exponent is inexact (`pow_dims(d, None)`): a
dimensionless base stays dimensionless, and a dimensioned one is refused as
`DimensionError.InexactExponent`.

`sin`, `cos`, `tan`, `exp`, `log`, `sinh`, `cosh` and `tanh` take a
dimensionless argument and give a dimensionless result
(`transcendental_dims`, refused as `DimensionError.NotDimensionless`).
`atan2(y, x)` is the same rule applied to `y / x`, so `y` and `x` need equal
dimensions.

Unsupported SymPy nodes, including derivatives, integrals, piecewise
expressions, Kronecker deltas, and nested relational expressions, raise
`UnsupportedExpressionError` rather than being silently accepted.

## Buckingham Pi API

The Pi API works directly on dimension dictionaries and integer exponents. It
checks and generates dimensionless monomial products without adding value-bearing
quantities, unit conversion, code generation, or third-party dependencies.

```python
from bridgman import count_pi_groups, is_dimensionless_product, pi_groups

rho = {"M": 1, "L": -3}
velocity = {"L": 1, "T": -1}
length = {"L": 1}
dynamic_viscosity = {"M": 1, "L": -1, "T": -1}

quantities = {
    "rho": rho,
    "v": velocity,
    "L": length,
    "mu": dynamic_viscosity,
}

assert count_pi_groups(quantities) == 1
assert is_dimensionless_product(
    quantities,
    {"rho": 1, "v": 1, "L": 1, "mu": -1},
)
assert pi_groups(quantities) == ({"rho": 1, "v": 1, "L": 1, "mu": -1},)
```

- `PiError`: raised when product names or quantity labels are invalid.
- `is_dimensionless_product(quantities, exponents)`: checks whether a
  user-authored integer power product is dimensionless.
- `count_pi_groups(quantities)`: returns the Buckingham count `n - rank(A)`.
- `pi_groups(quantities)`: returns Bridgman's deterministic integer basis for
  dimensionless power products.

Generated bases are useful diagnostics, not semantic identity surfaces.
Different valid bases can span the same dimensionless space, so downstream
systems should store original quantities plus checked authored products when
they need stable artifacts. Pi groups are dimension-only; they do not replace
the kind layer and cannot distinguish dimensional twins such as energy and
torque.

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
  rationale=None)`: declares a row that chooses between twins, with `op` one of
  `"mul"`, `"div"`, `"dot"` or `"wedge"`. A rule is kept only when derivation
  leaves two or more kinds with the product's dimensions and grade; a rule that
  restates what derivation resolves is refused. Set `commutative=True` to
  register both argument orders for multiplication (division rules cannot be
  commutative). `rationale` is an optional human string surfaced in
  `CheckResult.steps`.
- `KindRegistry(kinds=[...], rules=[...])`: writes the declarations as a
  catalog document, which the Rust core reads through its catalog schema and
  compiles. The core derives products, quotients and rational powers from
  dimensions and grade. `KindRegistry.bundled()` is the catalog Bridgman
  bundles (`catalogs/thermal.yml`), as the Rust core compiles it.
  Methods, each a Rust `Term` operation, where a term is a kind name or `None`
  for a pure number: `result_kind(left, op, right)` (`Term::combine`, for
  `add`, `sub`, `mul`, `div`, `dot` and `wedge`), `power_kind(base, exponent)`
  (`Term::power`, for an `int`, a `Fraction`, or `None` for an inexact
  exponent), `absolute_kind(term)` and `same_kind(left, right)`. Also
  `kind_names()`, `kind_dimensions(name)`, `rule_rationale(left, op, right)`,
  `kinds_with_dimensions(d)`, and `ambiguous_kinds(d)`.
- `kind_of_expr(expr, registry=..., kind_map=...)`: infers the semantic kind of
  a SymPy expression. A pure number has no kind: multiplying or dividing by one
  keeps a kind, `1/x` is `x` to the power `-1`, and a number in a sum or
  comparison with a quantity is refused as `QuantityError.NumberTerm`.
- `verify_expr_kinds(eq, registry=..., kind_map=...)`: verifies that both sides
  are the same kind (`Term::same`).
- `explain_expr_kinds(eq, registry=..., kind_map=...)`: returns a structured
  `CheckResult` with kinds, dimensions, reason text, and operation steps. A
  refusal's reason is `"<class>: <message>"`, for example
  `QuantityError.NoProductKind: no kind has dimensions M:1,L:1,T:-1 ...`.

### Errors

Every refusal is the Rust core's, raised as the Python class of its Rust error
variant. The classes are generated from the Rust enums when the extension
loads; each enum is a class deriving from `BridgmanError`, and each variant a
subclass set on it by name:

- `CatalogError`: a catalog cannot be read or compiled, e.g.
  `CatalogError.Duplicate`, `CatalogError.ConflictingOperationRule`,
  `CatalogError.InvalidOperationRule`, `CatalogError.DerivedOperationRule`.
- `QuantityError`: an operation on a compiled catalog's kinds is refused, e.g.
  `QuantityError.NoProductKind`, `QuantityError.UnresolvedTwin`,
  `QuantityError.UnresolvedPowerTwin`, `QuantityError.KindMismatch`.
- `DerivationError`: why derivation gives no kind (`Unknown`,
  `UnresolvedDimensions`, `Point`, `Ungraded`). It is raised as the `__cause__`
  of `CatalogError.Derivation` or `QuantityError.Derivation`, the variant that
  wraps it.
- `DimensionError`: a dimension rule or a signature is refused
  (`Unequal`, `NotDimensionless`, `InexactExponent`, `InvalidSignature`,
  `InvalidPower`).
- `OperationParseError`: an operation name the core does not know.

Each family lists its variants in `variants`. An exception's message is the
Rust error's, and its `fields` attribute holds what the variant names, with
kinds and units by id:

```python
from bridgman import KindRegistry, QuantityError

try:
    KindRegistry.bundled().same_kind("energy", "torque")
except QuantityError.KindMismatch as refused:
    assert refused.fields == {"expected": "energy", "actual": "torque"}
```

None of these is a `ValueError`. `UnsupportedExpressionError` (SymPy
structure), `PiError` (Pi inputs) and `SympyRequiredError` are Python's own.

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

## Native core and quantity catalogs

Bridgman 0.3 builds a Rust core and a maturin/PyO3 extension. The existing
Python dimension, kind and Pi APIs remain available; optional SymPy traversal
stays in Python and delegates dimension and kind operations to the extension.
The Rust `bridgman-core` crate can be used without Python.

The Rust core has one quantity engine. A `Catalog` declares kinds, units and
twin rows as data; products are derived. `Registry::compile` (or
`from_json`/`from_yaml`) checks it once and hands out `Kind` and `Unit`
handles. A `Quantity` is a finite value of a kind, and every arithmetic
operation asks `Kind::combine` which kind results. Kind identity is distinct
from dimensions. A product's kind is derived from dimensions and each kind's
grade in G3; a declared twin row only chooses between kinds that derivation
cannot tell apart. A catalog may name its `time` point kind, whose difference
kind is the duration; a kind declaring `rate_of: K` times a duration is `K`. Affine point/difference relationships, twin rows, the
dimensionless kind and a kind's least value (absolute zero, zero mass) are
declared, not compiled in; `Kind::minimum` reads a floor as a quantity. Handles carry their registry, so
mixing registries is refused, and ambiguous symbols require an explicit unit
selection. Unknown dimensions remain inspectable but cannot construct a
numerical quantity.

Each rule has one statement. `derive` is the product rule on dimensions and
grade alone (a point kind takes part in no product; G3 must give the product
one grade), and `Kind::product` resolves a kind from it. Its factors are a
kind's `Operand` or an already-derived `Graded` (`Factor::Kind`,
`Factor::Derived`), so products chain without giving a derived result a role. `Kind::difference` is the
kind of a difference, `Kind::power` takes an `Exponent` (exact and rational,
so a root is a power; an inexact one is refused), `Kind::scaled` says a pure
number scales every kind but a point, and `Kind::same` that values compare
only within one kind. `Term` (a pure number or a value of a kind) carries the
number rules an expression walker needs: `combine`, `power`, `absolute` and
`same`, refusing a number in a sum or comparison with a quantity as
`NumberTerm`. `Dimensions::common` (terms added or compared share their
dimensions), `Dimensions::transcendental` (exp, log, sin, ... take and give
dimension one) and `Dimensions::raised` (only dimension one survives an
inexact exponent) state the dimension rules, and `SI_BASES` the SI base order.
`AffineRole::offset` says a unit's offset applies to points only. A `QuantityError` names every
kind and unit by its handle, and `DerivationError` is the one statement of why
derivation gives no kind, wrapped by both `CatalogError` (which names kinds by
declared id, since no registry exists yet) and `QuantityError`. A `Quantity`
has no `==`: its value is binary64 in whichever reference unit it is held in,
so `Quantity::equals_exactly` compares two values of one kind exactly: each
held binary64 value is read as its exact rational, and the other is carried
into this one's unit by the ratio of the two units' exact coherent scales (not
by their conversion references). It refuses two kinds, and a point held in two
different references.

QUDV schema-2 catalogs can be imported with `qudv_schema2_to_catalog`.
Source IDs are scoped by the source hash, and correction provenance is retained.
Conversion reference scales and coherent-basis scales are separate: a gram
reference must not be mistaken for the coherent mass unit during products.
Products without a declared coherent scale fail explicitly. Approximate
conversion records are not available through the exact-conversion API.
What a catalog cannot hold is refused rather than dropped: an approximate or
non-monomial SI factor, an empty symbol, and a pi exponent that is not an
exact integer are errors, and a wrong schema version is `QudvSchema`.
The full OMG source/catalog is not bundled; callers supply their own artifact.

`catalogs/thermal.yml` is the thermal and mechanics catalog bundled for
Physica: an ordinary catalog that `bridgman_core::thermal()` reads and compiles
once per process, so every crate that reads it holds handles of one registry.
How a document writes its kinds and quantities is the consumer's vocabulary,
not Bridgman's.
Numerical quantities use finite binary64; exact conversion values retain
arbitrary-size rationals and powers of pi. Physical-law validity belongs to the
consumer.

Development checks:

```text
cargo test -p bridgman-core --locked
uv run --extra sympy pytest -q
uv run pyright
```

The Rust minimum is 1.85. The Python wheel uses `abi3-py310`; CI builds and tests
installed wheels on Windows and Linux with Python 3.10 and 3.13. On Windows,
set `PYO3_PYTHON` to a 64-bit interpreter before `cargo test --workspace` if
the first Python on PATH is 32-bit.

## Rewrite plan

[Rust rewrite plan](RUST_REWRITE_PLAN.md) records the quantity/catalog boundary,
sequential delivery milestones, and compatibility checks. Its
[acceptance cases](design/quantity-contract-cases.yml) are executable review
fixtures for the native implementation.

## License

MIT
