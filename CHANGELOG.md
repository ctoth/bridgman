# Changelog

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
