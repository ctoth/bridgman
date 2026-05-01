# Changelog

## Unreleased

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
