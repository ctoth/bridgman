# Rust Bridgman: preparation and delivery plan

Status: proposed implementation contract, not implemented. Investigated 2026-09-21.
Q authorized the preliminary work for a Rust-centered rewrite, built outward
into Physica. This supersedes the earlier workstream's exclusion of value-bearing
quantities and conversion for the new API; it does not silently change old APIs.

## Outcome and boundary

Bridgman supplies quantity semantics to both Rust and Python. Physica can load a
corrected catalog and execute its heating, melting and cooling declarations
without maintaining another list of quantity kinds, units or arithmetic rules.
Existing dimensional-analysis consumers keep their supported Python contracts.

| Owner | Responsibility |
|---|---|
| qudv-iso80000 | Interpret XMI, reconcile source evidence, apply reviewed corrections, emit source provenance and unresolved findings |
| Bridgman | Exact dimension algebra, kind identity and declared operation judgments, linear/affine quantity arithmetic and conversion, dimensional expression checks, Buckingham Pi |
| Physica | Physical laws and applicability, model bindings, Institution interpretation/composition, solve/evolve/check/observe and numerical evidence |
| Propstore | Contextual assertions, lexical/form bindings and model selection; existing Pint conversion remains until a separately tested migration |
| Conservation / Pyspace | Authoritative exchanges / objects, scheduling and domain policy |

Bridgman does not depend on Physica or Institution. It does not acquire a law
solver, physical-state inventory, source downloader, reaction database or array
engine. A common quantity operation is owned here; the AST and isolation of a
physical equation remain Physica's. Python's SymPy adapter translates supported
nodes into calls to shared Rust judgments; it does not implement those judgments
again or require SymPy in a Rust consumer.

## Observed baseline

- Bridgman `5eb6118`: Python 0.2.0, `uv_build`, Python >=3.9, no Cargo project.
  `uv run --extra sympy pytest -q`: **182 passed** on the local Python 3.13
  environment. This is not evidence for the whole supported Python matrix.
- Converter `16da5e1`: schema 2, correction manifests with source-hash/old-value
  checks, exact affine composition, topological required dependencies and
  provisional grounded alternative inference. Previous same-session full-XMI
  verification: **62 passed**. Its dirty pyproject/lock typing changes belong to Q.
- Physica `e9a5632`: declared regimes, law inclusion through signature morphisms,
  domain-bearing signatures, generic recurrence operations. Previous same-session
  verification: **34 integration tests and 5 doctests passed**.
- Bridgman has existing untracked notes and project directories; preserve them.
  No open issues were returned by `gh issue list` for ctoth/bridgman.

## Existing capabilities and disposition

| Surface | Preserve / change |
|---|---|
| `Dimensions`, mul/div/pow, equality, dimensionless checks | Preserve Python dictionary API; implement arithmetic once in Rust |
| `canonicalize_dims`, signatures, parsing, display | Preserve `Theta` aliases, zero removal, canonical serialized signatures and existing display behavior |
| `QuantityKind`, `OperationRule`, `KindRegistry` | Preserve names as caller-owned opaque identifiers, explicit mul/div rules, commutativity option, introspection and error classes; new registry extends this model |
| `CheckResult` and kind/dimension errors | Preserve public fields and exception hierarchy; add structured native causes without discarding old diagnostic information |
| `dims_of_expr`, `verify_expr`, `explain_expr` | Retain optional SymPy boundary and supported nodes, including exact roots and transcendental restrictions |
| Kind-aware symbolic variants | Retain explicit-rule semantics; dimensional equivalence never selects energy rather than torque implicitly |
| Pi product check/count/basis | Port exact rank/nullspace work; preserve deterministic results for the existing input order and fixtures |
| `py.typed`, root exports, no-SymPy stubs | Preserve and verify from installed artifacts |
| Removed `verify_equation` | Remains removed; do not resurrect an alias |
| Units, numeric quantities, affine roles, catalog import | New declared API, not previously supplied by Bridgman |

Some current behavior is deliberately narrower than the new core. `pow_dims`
rejects bool and non-integer powers. Symbolic rational powers are accepted only
when resulting exponents are integers. Keep these constraints in compatibility
entry points even if the new core represents rational exponents.

Do not accidentally normalize every old arithmetic input: currently
`canonicalize_dims` and signature functions normalize Theta aliases, while
`mul_dims`, `div_dims` and `dims_equal` operate on the supplied keys. Preserve
tested boundary behavior; any deliberate tightening gets its own migration note.
Unknown dimension keys are supported, so a seven-element SI array is insufficient.

## Verified consumers

| Checkout | Observed use and migration check |
|---|---|
| `../physgen/src/physgen/loader.py` | Imports `mul_dims`, `div_dims`; preserve dictionary shape and generated-dimension behavior |
| `../ecosim/src/ecosim/__init__.py` | Re-exports `Dimensions`, `canonicalize_dims`; pyproject pins older Bridgman `984c6e8` |
| `../propstore` at `da0b48b4` | `propstore/dimensions.py` imports canonicalization, equality, multiplication and integer powers; pyproject pins `5eb6118`; dimension signatures are used through Bridgman by boundary tests |
| Older Propstore worktrees / demo | Symbolic checks, explanations and signatures appear in older worktrees and `propstore-demos/physics/audit_script.py`; these are compatibility evidence, not claims about current main's implementation |
| `../physica/src/quantities.rs` | Prospective Rust consumer: 13 closed kinds, unit macro, dynamic `binary_kind`, typed operator declarations, finite quantities and series |

Current Propstore uses Pint for affine/logarithmic/delta and domain-unit handling.
Replacing Pint is NOT delivered by migrating Bridgman's current API. A separate
consumer campaign must establish coverage before deleting it. Likewise, do not
update sibling dependency pins as an incidental consequence of this rewrite.

## Decisions for the core

1. **Open identity.** A kind/unit/dimension has an opaque stable ID. Symbols and
   display names are aliases, never identity. Intern to validated handles when a
   registry is frozen; handles carry registry identity so unrelated catalogs
   cannot accidentally interoperate. Persist stable IDs and the catalog digest,
   never allocation-order indices. No mutable process-global registry.
2. **Exact dimensions.** Sparse ordered maps from dimension IDs to arbitrary-size
   rational exponents. An explicit SI profile maps source `Θ` to canonical
   `Theta`; unknown dimensions remain distinct. Null dimensions stay unresolved,
   unlike the empty map, which means dimension one.
3. **Exact conversion definitions.** Retain rational coefficients and integer
   powers of pi, including sums needed by offsets. Approximate coefficients stay
   labelled approximate. Compose affine transforms exactly where supported;
   evaluate into finite binary64 only at the numerical boundary. Use an
   established arbitrary-precision rational implementation, not f64 or i64 as a
   supposedly exact catalog representation. Unsupported exact operations remain
   explicit; this is not a new general symbolic simplifier.
4. **Reference frames for conversion.** A converter `reference_unit` is not
   necessarily coherent SI: gram can be the terminal reference. Store the
   reference identity with scale/offset and compose only connected, validated
   maps. Do not apply both `si_factor` and a reference conversion blindly.
5. **Kinds and units are separate.** A unit can measure several kinds. Quantity
   construction needs a kind or an unambiguous expected kind supplied by a law.
   Symbol `N*m` or matching dimensions alone cannot decide energy versus torque.
   Kind specialization is metadata, not automatic equivalence or substitutability.
6. **Affine roles are declared.** A point space names its difference kind.
   Point minus point gives a difference; point plus difference gives a point;
   generic point addition/scaling is refused. Point/difference role is not
   inferred from a unit's nonzero offset: kelvin points have zero offset and
   Celsius differences use the same scale without its offset. Future absolute
   thermodynamic-temperature products need an explicit absolute-origin
   interpretation, not a temperature-specific bypass in the core.
7. **Arithmetic meaning is declared once.** Validate kind rules against dimension
   arithmetic; retain the selected rule/provenance in diagnostics. Missing and
   conflicting rules are distinct. QUDV factor decompositions are evidence, not
   a license to synthesize every multiplication or cancellation rule. Inverse
   rules may be derived only from a declaration that permits the algebraic
   operation; numerical nonzero guards still apply.
8. **Runtime openness, bounded static conveniences.** Arbitrary loaded kinds use
   checked dynamic quantities. A selected bundled profile may generate Rust
   marker types/operators and Python stubs from the same declarations. Runtime
   extensions cannot magically acquire new compile-time Rust types or precise
   Python overloads. Keep Physica's existing compile-fail examples for the static
   profile and equivalent runtime rejection tests for dynamic catalogs.
9. **Domains stay with their meaning.** Finite arithmetic checks are numerical.
   Conversion invertibility and affine roles belong here. Material support,
   positive mass/specific heat and model applicability remain law declarations;
   do not make generic kelvin conversion enforce an unstated physical model.

## Catalog contract and ownership

Use a versioned source-independent registry document, with a QUDV schema-2 adapter
owned alongside the exporter. The Rust kernel accepts already resolved
declarations and does not reimplement the exporter's inference or corrections.
The adapter retains source hash, correction hash, source IDs and diagnostics;
Bridgman validates its imported view independently for internal consistency.

Minimum records: kind ID/dimensions/relationships; unit ID/symbol/kind associations;
reference conversion with exact scale/offset; affine point/difference relation;
operation declaration; provenance and unresolved status. Loading unresolved
records for inspection is allowed; constructing a numeric quantity that depends
on them returns a precise unresolved-data error. Catalogs are local immutable
artifacts. No source fetch or implicit correction occurs during law execution.

Cross-source equivalence and aliases require explicit mappings. Select only one
owner for each statement. The runtime registry is a compiled view of the
declarations, not a second manually maintained unit table. Generic operations
and parser syntax necessarily live in code; Celsius constants, thermal kind
rules and source corrections do not.

The accompanying `design/quantity-contract-cases.yml` is a reviewable acceptance
corpus, explicitly not an implemented API/schema. Turn it into a test harness
before freezing the registry wire format. Preserve QUDV's source-scoped identity
while using caller-owned IDs in the source-independent examples.

## Package layout and dependencies

Proposed workspace: `crates/bridgman-core` (Python-free Rust library),
`crates/bridgman-python` (PyO3 extension), existing `src/bridgman` (compatibility
and optional SymPy adapters). Publish the Python package under the existing
`bridgman` name, with `bridgman._core`, stubs and `py.typed` in the wheel.
Physica depends directly on the Rust core and never initializes Python.

Start with serde for registry interchange, arbitrary-precision rational support
(candidate `num-bigint`/`num-rational`), PyO3 and maturin. Pin actual versions and
MSRV only after a build spike validates Python 3.9 and current supported targets.
Prefer a stable-ABI wheel if its tested feature coverage permits it; do not claim
cross-platform support from a Windows build. The core's ordinary cargo tests
must not require Python tooling or a sibling checkout.

References checked during preparation:
- [Maturin configuration](https://www.maturin.rs/config): mixed project layout.
- [PyO3 distribution](https://pyo3.rs/main/building-and-distribution): interpreter
  and ABI configuration; select versions based on tested minimum support.
- [BigRational](https://docs.rs/num/latest/num/type.BigRational.html): established
  `Ratio<BigInt>` representation, avoiding a home-grown rational implementation.

## Sequential delivery tickets

These are local issue-ready scopes, not published issues. Finish, validate and
merge each before starting its dependent change; no stacked PRs.

| Order | Deliverable | Required evidence |
|---|---|---|
| B0 | Native packaging scaffold + dimension core + Python dictionary compatibility | Installed wheel outside checkout; dictionary/signature/error fixtures and property tests; Rust consumer without Python; test minimum interpreter |
| B1 | Native kind registry, expression judgments, optional SymPy translation, Pi helpers | Remaining 182-test baseline plus independent rank/nullspace and kind-collision checks; no-SymPy isolated environment; remove replaced Python algorithms |
| B2 | Versioned open registry and QUDV adapter | Corrected corpus loads all retained identities; unresolved generalized dimensions stay unresolved; aliases/foreign handles/schema errors fail precisely; no scalar-export dependency |
| B3 | Numeric quantities, exact linear/affine maps and declared operation profiles | Contract corpus below; forward/inverse/mixed-unit references, pi offsets, numerical overflow and incompatible-kind tests; no physical-name dispatch |
| B4 | Generated static profile and installed typing contract | Existing Physica compile-fail behavior, accepted/rejected Python type fixtures, runtime extension without Rust changes; generated artifacts reproducible |
| P0 | Replace Physica quantity ownership with Bridgman | Existing heating/melting/cooling examples and all question/morphism/refinement tests pass; remove closed Kind enum and duplicate arithmetic tables |
| P1 | Resume Physica installed wheel issue #4, then material issue #5 and coupled-region #7 | Installed native law execution; attributed phase data; two-body energy balance, refinement, bounded outputs and run reuse |

B0/B1 complete replacement of existing Bridgman behavior. B2-B4 add the quantity
capabilities Physica needs. They are one foundational rewrite with reviewable
milestones, not permission for an indefinite temporary second implementation.
Keep old code only until its corresponding native path earns parity, then delete
that code in the same delivery. Preserve consumer pins until integration tests
against the new installed wheel are run in isolated consumer environments.

## Separate findings and remaining decisions

- Physica's statement that satisfaction is total on in-domain models is too
  strong for its binary64 evaluator. A same-session probe supplied finite,
  individually valid heating variables with `m=cp=1e308`; multiplication yielded
  `Err(Quantity(NumericalFailure))` from `Equations.satisfies`. Clarify abstract
  satisfaction versus computable numerical evidence; do not turn this into false
  or hide it by ad hoc quantity limits. Track with Physica, not the catalog.
- Catalog construction still invokes the scalar converter; finish that boundary
  as part of B2 without replacing the newly delivered dependency algorithms.
- Source artifact redistribution terms and the reproducible release bundle need
  a concrete decision before shipping the full OMG-derived catalog in a wheel.
  B0/B1 and synthetic fixtures do not depend on that decision. Importing a
  user-supplied artifact remains useful; it must be sufficient for offline tests.
- Exact dependency versions, Python ABI and initial OS matrix are B0 spike
  outputs. No native packaging or performance claim has been established here.
- Existing workstream documents remain historical; this plan owns the rewrite.
  Refresh Physica's stale roadmap alongside P0, rather than appending another
  contradictory description of the original experiment.
