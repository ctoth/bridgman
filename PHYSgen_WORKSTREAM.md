# Physgen-Inspired Workstream

## Purpose

Turn the useful ideas from `../physgen` into a focused Bridgman workstream.
The goal is not to import Physgen's generator. The goal is to make Bridgman
better at answering physics questions by layering semantic quantity kinds,
operation intent, and richer validation on top of its existing dimension
arithmetic.

This workstream is ordered. Each phase depends only on prior phases.

## Source Material Reviewed

I did not read all of `../physgen`. The workstream is based on the parts that
were inspected:

- `../physgen/README.md`
- `../physgen/pyproject.toml`
- `../physgen/src/physgen/models.py`
- `../physgen/src/physgen/loader.py`
- `../physgen/src/physgen/type_safety.py`
- `../physgen/src/physgen/brandhash.py`
- `../physgen/languages/python/runtime.py.j2`
- `../physgen/languages/python/README.md.j2`
- `../physgen/example/physics.yml`
- `../physgen/test_specs/README.md`
- `../physgen/tests/test_loader_edge_cases.py`
- `../physgen/tests/test_brandhash.py`
- `../physgen/prototype/quantity_kinds.py`

## Target Architecture

Bridgman keeps its current dimension dictionary API as the arithmetic core.
The new surface adds semantic physics metadata above dimensions:

- `Dimensions`: exponent vectors over canonical base dimension names.
- `QuantityKind`: a semantic physical kind with a name and dimensions.
- `OperationRule`: a declared operation between kinds, with an expected result
  kind and optional rationale.
- `KindRegistry`: a collection of known kinds and operation rules.
- symbolic checks can stay dimension-only, or can opt into kind-aware checking
  when a registry and symbol-kind map are supplied.

Dimension equality remains necessary but not sufficient. Kind-aware checking is
what distinguishes examples such as:

- `Energy` vs `Torque`
- `Pressure` vs `EnergyDensity`
- `SpecificEnergy` vs `AbsorbedDose`
- `Frequency` vs `Activity`
- `Angle` vs plain `Unitless`

## Non-Goals

- Do not import Physgen as a dependency.
- Do not copy Physgen's multi-language generator.
- Do not add unit conversion or `Quantity(value, unit)` in this workstream.
- Do not introduce BrandHash until Bridgman has a serialized schema or unit
  catalog worth hashing.
- Do not replace the dict API with a `Dimensions` class in this workstream.

## Testing Doctrine

This workstream should be test-driven. Each phase starts by adding the tests
that describe the target behavior, watching them fail for the intended reason,
then making the smallest production change that turns them green.

Use example tests for named physics facts and regression cases. Use Hypothesis
for algebraic and structural properties where examples are too narrow.

### TDD Rules

- Write or update tests before production code for each work item.
- A failing test must fail for the behavior under work, not because imports or
  fixtures are broken.
- Keep every test that exposed a bug or semantic gap.
- Prefer one behavioral slice at a time:
  1. failing example or property,
  2. implementation,
  3. targeted test pass,
  4. full suite pass,
  5. commit.
- Do not mark a property test as xfail unless the corresponding missing
  capability is explicitly scheduled in a later phase.

### Hypothesis Strategy Surface

Add reusable strategies rather than ad hoc generated data inside each test:

- `dimension_maps`: canonical SI dimension maps with bounded integer exponents.
- `dimensioned_maps`: non-empty normalized dimensions.
- `dimensionless_maps`: maps that canonicalize to `{}`.
- `kind_names`: valid identifier-like kind names.
- `quantity_kinds`: unique names paired with generated dimensions.
- `operation_rules`: generated `mul` and `div` rules whose result dimensions are
  either valid by construction or intentionally invalid for negative tests.
- `registries`: generated `KindRegistry` values with no duplicate kind names
  and no duplicate operation keys.
- `sympy_dimension_exprs`: generated symbolic expressions whose dimensions are
  known by construction.
- `sympy_kind_exprs`: generated symbolic expressions whose kinds are known by
  registry rules.

### Core Properties

The workstream should prove these properties as the relevant APIs land:

- Dimension multiplication is associative and commutative under `dims_equal`.
- Dimension division cancels multiplication.
- Integer powers distribute over multiplication.
- Canonical signatures are stable under input order and round-trip parsing.
- `canonicalize_dims` is idempotent.
- Registry construction is order-insensitive for lookup behavior.
- Commutative operation rules behave the same regardless of operand order.
- Every accepted `OperationRule` is dimensionally consistent.
- Every rejected generated invalid rule fails closed with a typed error.
- Kind-aware expression dimensions agree with dimension-only expression
  dimensions for the same expression.
- Kind-aware addition is stricter than dimension-only addition for dimensional
  twins.
- Explanation APIs are observational only: calling `explain_*` does not change
  the boolean result of `verify_*`.

## Phase 1: Fix Current Symbolic Semantics

### Scope

Patch correctness gaps in the current symbolic layer before adding a semantic
kind layer.

### Work Items

1. Add failing example tests and Hypothesis-generated symbolic regression tests
   for the current semantic gaps.

2. Allow dimensionless bases raised to arbitrary symbolic or floating
   exponents.
   - Example: `2 ** x` is dimensionless regardless of `x`.
   - Dimensioned bases still require exact numeric exponents.

3. Correct `atan2` semantics.
   - `atan2(y, x)` should require `y` and `x` to have equal dimensions.
   - The result is dimensionless.
   - It should not require each argument to be individually dimensionless.

4. Support `Abs`.
   - `Abs(x)` preserves the dimensions of `x`.

5. Support `Min` and `Max`.
   - All arguments must have equal dimensions.
   - The result has that shared dimension.

6. Support inequalities in the same validation family as equality.
   - `Lt`, `Le`, `Gt`, and `Ge` require both sides to have equal dimensions.
   - The result of verification is true for dimensionally valid inequalities.

7. Add generated properties for the new symbolic forms:
   - `Abs(x)` always has the same dimensions as `x`.
   - `Min` and `Max` over same-dimension generated symbols return that
     dimension.
   - `Min` and `Max` over mixed generated dimensions fail.
   - `atan2(a, b)` accepts generated operands iff their dimensions are equal.
   - inequalities accept generated operands iff their dimensions are equal.

### Files

- `src/bridgman/symbolic.py`
- `tests/test_symbolic.py`
- `tests/test_transcendentals.py`
- `tests/test_symbolic_properties.py`

### Acceptance Checks

- `uv run pytest`
- `uv run pyright`
- Tests include regression coverage for:
  - `2 ** x`
  - dimensioned base with symbolic exponent still rejected
  - `atan2(length, length)` accepted
  - `atan2(length, time)` rejected
  - `Abs(length)` returns length dimensions
  - `Min(length_a, length_b)` and `Max(length_a, length_b)` accepted
  - mixed-dimension `Min` and `Max` rejected
  - inequality sides must match dimensions

## Phase 2: Add the Physgen Collision Corpus

### Scope

Add tests that name the semantic limitation of pure dimensional analysis. These
tests should document the problem before the kind-aware API is added.

### Work Items

1. Add fixtures for dimensional twins:
   - `Energy`: `{"M": 1, "L": 2, "T": -2}`
   - `Torque`: `{"M": 1, "L": 2, "T": -2}`
   - `Pressure`: `{"M": 1, "L": -1, "T": -2}`
   - `EnergyDensity`: `{"M": 1, "L": -1, "T": -2}`
   - `SpecificEnergy`: `{"L": 2, "T": -2}`
   - `AbsorbedDose`: `{"L": 2, "T": -2}`
   - `Frequency`: `{"T": -1}`
   - `Activity`: `{"T": -1}`
   - `Angle`: `{}`
   - `Unitless`: `{}`

2. Add tests showing dimension-only verification cannot distinguish those
   twins.

3. Add tests defining the desired kind-aware behavior, marked as skipped or
   xfail until Phase 3 lands.

4. Add a Hypothesis property that generates two distinct kind labels with the
   same dimensions and proves dimension-only equality cannot distinguish them.

5. Add a Hypothesis property that generated dimensional twins become distinct
   as soon as kind labels are considered.

### Files

- `tests/test_quantity_kinds.py`
- `tests/test_quantity_kind_properties.py`

### Acceptance Checks

- `uv run pytest`
- Existing behavior remains green.
- The skipped or xfailed tests clearly name the missing kind-aware capability.

## Phase 3: Add Kind and Operation Models

### Scope

Introduce the minimal semantic model. This should be a small runtime module,
not a generator and not a unit-conversion system.

### Proposed Public API

```python
from bridgman import KindRegistry, OperationRule, QuantityKind

registry = KindRegistry(
    kinds=[
        QuantityKind("Force", {"M": 1, "L": 1, "T": -2}),
        QuantityKind("Length", {"L": 1}),
        QuantityKind("Energy", {"M": 1, "L": 2, "T": -2}),
    ],
    rules=[
        OperationRule("Force", "mul", "Length", "Energy", rationale="Work: W = Fd"),
    ],
)
```

### Work Items

1. Add failing tests for construction, duplicate detection, lookup behavior,
   dimensional rule validation, and commutative rule behavior.

2. Add `QuantityKind`.
   - Fields: `name`, `dimensions`.
   - Dimensions are canonicalized at construction.

3. Add `OperationRule`.
   - Fields: `left_kind`, `op`, `right_kind`, `result_kind`, `commutative`,
     `rationale`.
   - Supported ops for this workstream: `mul`, `div`.

4. Add `KindRegistry`.
   - Validates duplicate kind names.
   - Allows dimensional twins with different kind names.
   - Rejects duplicate operation rules.
   - Validates every rule references known kinds.
   - Validates every rule is dimensionally consistent.

5. Add lookup helpers.
   - `kind_dimensions(kind_name)`
   - `result_kind(left_kind, op, right_kind)`
   - `kinds_with_dimensions(dimensions)`
   - `ambiguous_kinds(dimensions)`

6. Add Hypothesis registry properties:
   - registry lookup is independent of kind and rule input order.
   - accepted generated rules satisfy dimension arithmetic.
   - generated duplicate operation keys are rejected.
   - generated unknown references are rejected.
   - `kinds_with_dimensions(d)` returns exactly the generated kinds whose
     canonical dimensions equal `d`.

7. Export the new API from `bridgman.__init__`.

### Files

- `src/bridgman/kinds.py`
- `src/bridgman/__init__.py`
- `tests/test_kinds.py`
- `tests/test_kind_properties.py`

### Acceptance Checks

- `uv run pytest`
- `uv run pyright`
- Registry validates:
  - `Force * Length -> Energy`
  - commutative reverse rule generation
  - duplicate rule rejection
  - unknown kind rejection
  - dimensionally invalid rule rejection
  - Energy and Torque can share dimensions without collision

## Phase 4: Kind-Aware Symbolic Checking

### Scope

Make symbolic verification optionally semantic. Dimension-only APIs stay
available. Kind-aware APIs require a registry and a symbol-kind map.

### Proposed Public API

```python
from bridgman import verify_expr_kinds

verify_expr_kinds(
    sp.Eq(E, F * d),
    registry=registry,
    kind_map={"E": "Energy", "F": "Force", "d": "Length"},
)
```

### Work Items

1. Add failing example tests for each accepted and rejected kind-aware symbolic
   behavior before adding the inference walker.

2. Add generated expression strategies for small symbolic expression trees.

3. Add a kind inference walker for SymPy expressions.
   - Symbols use `kind_map`.
   - Numbers are dimensionless scalar values.
   - Addition and subtraction require identical kinds.
   - Multiplication and division use `KindRegistry` operation rules.
   - Powers initially support exact integer exponents only when the registry can
     find a unique result kind by dimensions.

4. Add a kind-aware verification entry point.
   - `kind_of_expr(expr, registry, kind_map)`
   - `verify_expr_kinds(eq, registry, kind_map)`

5. Add kind-aware errors.
   - Unknown symbol kind.
   - Unsupported operation rule.
   - Ambiguous result kind.
   - Dimensionally impossible declared rule.

6. Add Hypothesis properties:
   - kind inference dimensions agree with `dims_of_expr`.
   - generated valid operation trees infer the expected final kind.
   - generated missing operation edges fail closed.
   - generated same-dimension different-kind additions fail.

7. Preserve dimension-only `dims_of_expr` and `verify_expr`.

### Files

- `src/bridgman/kinds.py`
- `src/bridgman/symbolic.py`
- `src/bridgman/__init__.py`
- `tests/test_kind_symbolic.py`
- `tests/test_kind_symbolic_properties.py`

### Acceptance Checks

- `uv run pytest`
- `uv run pyright`
- Tests prove:
  - `Force * Length -> Energy`
  - `Length * Force -> Energy` when rule is commutative
  - `Energy + Torque` rejected despite equal dimensions
  - `Pressure + EnergyDensity` rejected despite equal dimensions
  - `Angle + Unitless` rejected despite both being dimensionless
  - dimension-only `verify_expr` behavior is unchanged

## Phase 5: Add Explanation Results

### Scope

Add an inspectable result object so users can see why a check succeeded or
failed.

### Proposed Public API

```python
result = explain_expr_kinds(eq, registry=registry, kind_map=kind_map)
assert result.ok
print(result.lhs_kind)
print(result.rhs_kind)
print(result.steps)
```

### Work Items

1. Add failing tests for explanation shape before adding `CheckResult`.

2. Add `CheckResult`.
   - `ok: bool`
   - `lhs_kind`
   - `rhs_kind`
   - `lhs_dimensions`
   - `rhs_dimensions`
   - `reason`
   - `steps`

3. Add dimension-only explanation if cheap.
   - `explain_expr(eq, dim_map)`

4. Add kind-aware explanation.
   - `explain_expr_kinds(eq, registry, kind_map)`

5. Keep boolean APIs as thin consumers of explanation APIs.

6. Add Hypothesis properties:
   - `explain_expr(...).ok == verify_expr(...)` for generated dimension-only
     equations that do not use unsupported nodes.
   - `explain_expr_kinds(...).ok == verify_expr_kinds(...)` for generated
     kind-aware equations.
   - explanations for generated failures include at least one concrete
     mismatch or missing-rule reason.

### Files

- `src/bridgman/symbolic.py`
- `src/bridgman/kinds.py`
- `tests/test_explain.py`
- `tests/test_explain_properties.py`

### Acceptance Checks

- `uv run pytest`
- `uv run pyright`
- Tests assert explanations include:
  - mismatched dimensions for dimension-only failures
  - mismatched kinds for dimensionally equal twins
  - missing operation rule details
  - operation rationale when a rule is used

## Phase 6: Declarative Physics Fixtures

### Scope

Steal Physgen's useful test-spec idea without adding a generator. Add a small
YAML or Python fixture format that describes kind definitions, operation rules,
and symbolic equations.

### Work Items

1. Add schema validation tests before adding fixture loading.

2. Add a compact fixture file for mechanics.
   - `Mass`, `Length`, `Time`, `Velocity`, `Acceleration`, `Force`, `Energy`,
     `Power`, `Momentum`, `Torque`, `Angle`, `Unitless`.

3. Add a compact fixture file for collisions.
   - Energy/Torque, Pressure/EnergyDensity, Frequency/Activity,
     SpecificEnergy/AbsorbedDose, Angle/Unitless.

4. Add loader helpers for tests only unless there is a clear public use.

5. Generate tests from fixtures inside pytest.

6. Add Hypothesis schema properties:
   - generated valid fixture fragments load into equivalent registries.
   - generated invalid fixture fragments fail closed.
   - fixture order does not change registry behavior.

### Files

- `tests/fixtures/kinds_mechanics.yml`
- `tests/fixtures/kinds_collisions.yml`
- `tests/test_kind_fixtures.py`
- `tests/test_kind_fixture_properties.py`

### Acceptance Checks

- `uv run pytest`
- Fixture validation fails closed:
  - unknown kind
  - duplicate kind
  - duplicate operation
  - dimensionally invalid rule

## Phase 7: Documentation and Release Surface

### Scope

Document the new semantic layer as a deliberate addition, not a replacement for
dimension arithmetic.

### Work Items

1. Add public API export tests and README example tests before documentation
   edits are considered complete.

2. Update README.
   - Explain dimensions vs kinds.
   - Show the Energy vs Torque problem.
   - Show kind-aware verification.
   - State that the dict API remains the arithmetic core.

3. Update CHANGELOG.
   - Mention kind registry.
   - Mention kind-aware symbolic verification.
   - Mention new supported SymPy forms from Phase 1.

4. Add API export tests.
   - New public names appear in `bridgman.__all__`.

5. Add optional usage examples.
   - `Force * Length -> Energy`
   - `Energy + Torque` rejected

6. Keep a lightweight property test that imports every public symbol listed in
   `__all__` and verifies it resolves to an attribute on `bridgman`.

### Files

- `README.md`
- `CHANGELOG.md`
- `tests/test_public_api.py`

### Acceptance Checks

- `uv run pytest`
- `uv run pyright`
- `uv build`

## Deferred Work

These ideas are worth keeping, but they do not belong in this workstream.

### Quantity Values and Units

Physgen has `Quantity(value, kind, dimension)` and unit constructors. Bridgman
should only add that after kind-aware symbolic checking is stable.

### Affine and Gauge Quantities

Temperature and electric potential need absolute-value vs delta semantics.
This should become a separate workstream after the kind registry exists.

### BrandHash

BrandHash is useful only once Bridgman serializes a schema, unit catalog, or
quantity data. Until then, it adds protocol surface without a payload.

### Generated Packages

Physgen's generator is outside Bridgman's scope. Bridgman should remain a small
library for dimension arithmetic and symbolic validation.

## Definition of Done

The workstream is complete when all of these are true:

- Current symbolic correctness gaps are fixed.
- Physgen's dimensional twin corpus exists in Bridgman tests.
- `QuantityKind`, `OperationRule`, and `KindRegistry` are public APIs.
- Kind-aware symbolic verification rejects dimensionally equal but semantically
  different quantities.
- Explanation APIs identify dimension mismatches, kind mismatches, missing
  operation rules, and successful rule rationales.
- README documents the distinction between dimensions and kinds.
- `uv run pytest`, `uv run pyright`, and `uv build` pass.
