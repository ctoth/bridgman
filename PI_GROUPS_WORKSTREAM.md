# Buckingham Pi Workstream

## Purpose

Build the smallest useful Kennedy-inspired extension to Bridgman: helpers for
dimensionless products and Buckingham-Pi counting/generation over existing
integer dimension dictionaries.

The goal is not to implement Kennedy's full unit-polymorphic type system.
Bridgman should remain a small library of dimensional judgments. This
workstream adds one narrow capability: given named quantities and their
dimensions, prove or generate dimensionless monomial groups.

## Source Material

- `../propstore/papers/Kennedy_1997_RelationalParametricityUnitsMeasure/notes.md`
- `../physgen/research/06-si-bipm-standards.md`
- `../physgen/research/synthesis.md`
- `PHYSgen_WORKSTREAM.md`
- `src/bridgman/dimensions.py`
- `src/bridgman/kinds.py`
- `tests/test_propstore_consumer_contract.py`

## Target Architecture

Add a new public module, `bridgman.pi`, exported from `bridgman.__init__`.

Public API:

```python
from bridgman import (
    PiError,
    count_pi_groups,
    is_dimensionless_product,
    pi_groups,
)
```

### `is_dimensionless_product`

```python
def is_dimensionless_product(
    quantities: Mapping[str, Dimensions],
    exponents: Mapping[str, int],
) -> bool:
    ...
```

Returns whether the product of named quantities raised to integer exponents is
dimensionless.

This is the core proof-checking function. It is canonical, deterministic, and
safe for downstream systems to use in diagnostics.

Example:

```python
rho = {"M": 1, "L": -3}
velocity = {"L": 1, "T": -1}
length = {"L": 1}
mu = {"M": 1, "L": -1, "T": -1}

assert is_dimensionless_product(
    {"rho": rho, "v": velocity, "L": length, "mu": mu},
    {"rho": 1, "v": 1, "L": 1, "mu": -1},
)
```

### `count_pi_groups`

```python
def count_pi_groups(quantities: Mapping[str, Dimensions]) -> int:
    ...
```

Returns `n - rank(A)`, where `A` is the dimension matrix with one column per
quantity and one row per dimension key. This count is the canonical part of
Buckingham's theorem and is safe to use in tests and user-facing diagnostics.

### `pi_groups`

```python
def pi_groups(quantities: Mapping[str, Dimensions]) -> tuple[dict[str, int], ...]:
    ...
```

Returns a deterministic integer basis for dimensionless monomials. Each result
is a mapping from quantity name to integer exponent.

This basis is deterministic for Bridgman's implementation, but it is not a
semantic identity surface. Different valid bases can span the same Pi space.
Consumers must not use generated basis vectors as stable artifact IDs or
equivalence keys. If a stable artifact is needed, store the original quantities
plus a checked user-authored product and verify it with
`is_dimensionless_product`.

## Hard Boundaries

- Do not add value-bearing `Quantity` objects.
- Do not add unit conversion.
- Do not add code generation.
- Do not depend on `physgen` or `propstore`.
- Do not use generated Pi groups to decide equation equivalence.
- Do not replace or weaken `QuantityKind`; Pi groups are dimension-only and
  cannot distinguish dimensional twins such as Energy/Torque.
- Do not add a required dependency. Implement integer/rational linear algebra
  with the standard library, or keep the API unavailable without an optional
  dependency. The preferred target is standard-library-only.

## Implementation Notes

- Dimension dictionaries remain `dict[str, int]`.
- Quantity labels are opaque non-empty strings. They do not need to be Python
  identifiers and may be Propstore concept IDs.
- Reject unknown quantity names in `exponents`.
- Reject bool and non-int exponents in both dimension maps and product maps.
- Canonicalize dimensions before matrix construction.
- Use `DIM_ORDER` first and lexicographic fallback for row order.
- Use input mapping order for quantity column order, but normalize each basis
  vector so the first non-zero exponent is positive and the vector is reduced
  by the greatest common divisor of absolute coefficients.
- Empty and all-dimensionless inputs are valid. `count_pi_groups` returns the
  number of quantities. `pi_groups` returns one unit vector per input quantity.
- If there are no Pi groups, return an empty tuple.

## Tests First

### Phase 1: Proof Checking

Add `tests/test_pi_groups.py`.

Required examples:

- Reynolds-number product is dimensionless:
  `rho * v * L / mu`.
- `v * L / mu` is not dimensionless when density is omitted.
- Unknown product names raise `PiError`.
- Bool and non-int product exponents raise `TypeError`.
- Quantity labels such as `ps:concept:density` are accepted.

Acceptance:

```powershell
uv run pytest tests/test_pi_groups.py
```

### Phase 2: Canonical Count

Add examples for Buckingham counts:

- Pendulum variables `t, l, g, m, theta` have 2 Pi groups when dimensions are
  `T`, `L`, `L T^-2`, `M`, and dimensionless angle.
- Reynolds variables `rho, v, L, mu` have 1 Pi group.
- `Force, mass, acceleration` have 1 Pi group because `F / (m*a)` is
  dimensionless.
- Independent base quantities `M, L, T` have 0 Pi groups.

Add property tests:

- `count_pi_groups(quantities)` is invariant under quantity insertion order.
- The count is between `0` and `len(quantities)`.

Acceptance:

```powershell
uv run pytest tests/test_pi_groups.py tests/test_dimension_properties.py
```

### Phase 3: Basis Generation

Add examples:

- Reynolds variables produce one checked group equivalent to
  `rho * v * L / mu`.
- Each generated group passes `is_dimensionless_product`.
- Reordering the input mapping preserves `count_pi_groups`, but callers must not
  rely on identical generated basis vectors. The test should assert documented
  behavior only.
- Energy/Torque/Angle produce dimension-only Pi groups, while
  `verify_expr_kinds` still rejects Energy/Torque mixing. This proves Pi groups
  do not replace the kind layer.

Add properties:

- Every generated group is dimensionless.
- Number of generated groups equals `count_pi_groups`.
- Generated vectors are reduced by coefficient GCD and have a positive first
  non-zero exponent.

Acceptance:

```powershell
uv run pytest
uv run pyright
uv build
```

## Downstream Contract Tests

After Bridgman passes locally, Propstore can consume the pushed commit only
after it exists on a remote. The consumer pin must never be local.

Minimum Propstore verification after a pin bump:

```powershell
uv run pytest tests/test_form_dimensions.py tests/test_equation_comparison.py tests/test_equation_comparison_properties.py tests/test_bridgman_signal_propagation.py
```

No Propstore source changes should be necessary for the initial Bridgman
release because the Pi API is additive.

## What This Unlocks

For Bridgman:

- a first-class dimensional-invariance helper;
- a precise Kennedy/Buckingham-backed feature without becoming a full type
  system;
- stronger examples for explaining why dimensions are algebra, not just labels.

For Propstore:

- equation diagnostics can say how many independent dimensionless groups an
  equation family admits;
- authored Pi products can be verified as claims;
- scale-invariance checks can become provenance-bearing diagnostics later.

For Physgen:

- generated language examples can include dimensionless groups such as Reynolds
  number without hard-coding them as one-off facts;
- test specs can distinguish dimensional validity, semantic quantity-kind
  validity, and dimensionless-product validity.

## Non-Goals For This Workstream

- No automatic law discovery.
- No claim that same Pi groups imply equation equivalence.
- No canonical physics naming of Pi groups.
- No affine units or gauge quantities.
- No ISO 80000 registry import.
- No BrandHash or serialized registry schema.

## Definition Of Done

- `PiError`, `is_dimensionless_product`, `count_pi_groups`, and `pi_groups` are
  public exports.
- All examples and properties above pass.
- README documents the API with the Reynolds-number example and the warning
  that generated bases are not semantic identity surfaces.
- Existing dimension, symbolic, kind, and Propstore consumer tests still pass.
