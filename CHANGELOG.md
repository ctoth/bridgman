# Changelog

## Unreleased

- The Python package derives kind arithmetic from the Rust core: products,
  quotients and integer powers come from dimensions and grade, and a rule only
  chooses between twins. `KindRegistry.operation_rule` and
  `unique_kind_with_dimensions` are removed; `power_kind`, `rule_rationale`,
  `kind_names`, `KindRegistry.bundled()` and `DerivedOperationRuleError` are
  added. Rust gains `Kind::power`, `Kind::row_provenance`, `Grade::power`,
  `Operation::Power` and
  `QuantityError::{NoPowerKind, UngradedPower, UnresolvedPowerTwin}`.
- The Python extension is built with PyO3 0.29, which fixes an out-of-bounds
  read in `PyList`/`PyTuple` iterators and a missing `Sync` bound on closures.
- Python 3.9, end of life since October 2025, is no longer supported: the
  package requires Python 3.10 and the wheel uses `abi3-py310`.
- Added semantic quantity kinds with `QuantityKind`, `OperationRule`, and
  `KindRegistry`.
- Added kind-aware symbolic APIs: `kind_of_expr`, `verify_expr_kinds`, and
  `explain_expr_kinds`.
- Added structured `CheckResult` explanations for dimension-only and
  kind-aware symbolic validation.
- Added support for `Abs`, `Min`, `Max`, and inequalities in symbolic
  dimensional analysis.
- Corrected `atan2(y, x)` semantics so both operands must have equal
  dimensions rather than being individually dimensionless.
- Allowed dimensionless bases to have symbolic or floating exponents while
  preserving exact-exponent requirements for dimensioned bases.
- Rust catalog schema 2: a unit's `conversion` groups `reference_unit`,
  `scale` and `offset`; an approximate magnitude is `{"approximate": x}` and an
  exact offset is a list of terms. Declared operations accept only `mul` and
  `div`. A kind's affine role follows from its declared affine space and is no
  longer passed to `Registry::quantity`, `quantity_for_symbol` or
  `convert_exact`. Catalog errors are structured variants.
- The Python dimension functions use the Rust `Dimensions` type. Dimension
  dictionaries are returned in signature order (M, L, T, I, Theta, N, J, then
  other identifiers), and `parse_dims_signature` is implemented natively.
- One Rust quantity engine (catalog schema 3). The generated closed profile
  (`Kind` enum, typed `Quantity<K>`/`Unit<K>` constants, `binary_kind`, its own
  `QuantityError`) and `tools/generate_profile.py` are removed;
  `profiles/thermal.yml` is now an ordinary catalog read by
  `profile::registry()`. `Kind<'r>` and `Unit<'r>` handles carry their
  registry, `Quantity<'r>` replaces `DynamicQuantity`, and `Kind::combine` is
  the one statement of kind arithmetic. Catalogs may declare a `dimensionless`
  kind and a kind's `minimum`. Refused affine arithmetic is
  `UnsupportedOperation` (Python tag `unsupported_operation`); `sqrt` and the
  profile's root rule are gone. Faults of a catalog (reading, importing,
  compiling) are `CatalogError`; `QuantityError` is left for refused operations.
- Catalog schema 4: products derived from dimensions and grade; `operations`
  holds only twin rows. A kind declares its `grade` in G3 (0 to 3, default
  0), and `dot` and `wedge` join `mul` and `div`. A declared row is kept only
  when derivation leaves two or more kinds and the row names one of them; a
  row that restates a derivation is `DerivedOperationRule`, and one whose
  result has other dimensions or grade is `InvalidOperationRule` (now carrying
  the row and the derived dimensions and grade). A commutative division is
  `CommutativeQuotient`, a row with no single grade `UngradedOperationRule`,
  and a row naming a point kind `PointOperationRule`. `MissingOperationRule` is
  removed: a product no kind has is `NoProductKind`, two non-scalars under
  `mul` are `UngradedProduct`, and twins without a row are `UnresolvedTwin`.
  The thermal profile's 13 rows are all derived and are removed. Python
  `KindRegistry` refuses a rule that derivation resolves with the tag
  `derived_rule`.
- `Kind::minimum` is public and returns the floor as a `Quantity` in the
  kind's canonical unit. `BelowMinimum` names the unit, the exact floor and
  the exact offending value (Python tag `below_minimum` carries them encoded).
  The thermal profile declares a floor of 0 for mass.
- A catalog may name its `time` kind, a scalar point kind whose difference
  kind is the duration rates are taken over (`InvalidTimeKind` otherwise). A
  kind may declare `rate_of: <kind>`: it times a duration is that kind, which
  derivation uses to choose between twins (and a duration divides the kind
  back to its rate). Faults are `InvalidRate` with a `RateFault`.
  `Registry::time`, `Kind::rate_of` and `Kind::rate` read them. In the thermal
  profile `time` is a point kind with difference kind `duration`; instants are
  written `s` and durations `delta_s`.
- The thermal profile declares `enthalpy`, a point kind whose differences are
  `energy` (written `enthalpy_J` and `enthalpy_kJ`); `energy` is therefore a
  difference kind.
- The bundled profile gains mechanics kinds: `displacement` (grade 1,
  `vec_m`), `velocity`, `acceleration`, `force` and `momentum` (grade 1),
  `power`, `angular_momentum` and `angle` (grade 2), `angular_velocity`
  (grade 2) and `frequency`, with their units; `N*s` is a unit of momentum.
  Each rate names what it is the rate of, so the profile needs no twin rows:
  force·displacement under `dot` is energy and under `wedge` is torque.

## v0.2.0

- Added dimensionless-argument dispatch for symbolic transcendental functions:
  `sin`, `cos`, `tan`, `exp`, `log`, `sinh`, `cosh`, `tanh`, and `atan2`.
- Changed unsupported symbolic nodes from raw `TypeError` to `DimensionalError`,
  so consumers can distinguish wrong Python input from dimensional rejection.
- Added a runtime integer guard to `pow_dims`.
- Added `dims_signature` and `parse_dims_signature` for canonical dimension
  signatures.
- Added `canonicalize_dims`, including uppercase/lowercase theta glyph mapping
  to `Theta`.
- Deleted `verify_equation` in the same release as the `verify_expr` migration.
  This is the D-13 / Codex 2.27 one-shot deletion: no deprecation period and no
  compatibility shim.
