# Changelog

## Unreleased

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
