"""Semantic quantity kinds layered over dimension arithmetic."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, Literal

from bridgman._core import NativeKindRegistry

from bridgman.dimensions import Dimensions, canonicalize_dims, parse_dims_signature


OperationName = Literal["mul", "div", "dot", "wedge"]


class KindError(Exception):
    """Base class for quantity-kind validation errors."""


class DuplicateKindError(KindError):
    """Raised when a registry contains the same kind name twice."""


class DuplicateOperationRuleError(KindError):
    """Raised when a registry contains the same operation key twice."""


class UnknownKindError(KindError):
    """Raised when a kind name is not present in a registry."""


class InvalidOperationRuleError(KindError):
    """Raised when an operation rule is unknown or dimensionally invalid."""


class MissingOperationRuleError(InvalidOperationRuleError):
    """Raised when no operation rule exists for a requested operation."""


class DerivedOperationRuleError(InvalidOperationRuleError):
    """Raised when a declared rule restates a product that derivation resolves."""


class AmbiguousKindError(KindError):
    """Raised when dimensions match multiple semantic kinds."""


class KindMismatchError(KindError):
    """Raised when semantic kinds are incompatible."""


def _require_non_empty_string(value: str, field: str) -> None:
    if not isinstance(value, str) or value == "":
        raise ValueError(f"{field} must be a non-empty string")


@dataclass(frozen=True)
class QuantityKind:
    """A semantic quantity kind with dimensions."""

    name: str
    dimensions: Dimensions

    def __post_init__(self) -> None:
        _require_non_empty_string(self.name, "QuantityKind.name")
        object.__setattr__(self, "dimensions", canonicalize_dims(self.dimensions))


@dataclass(frozen=True)
class OperationRule:
    """A declared operation between semantic quantity kinds."""

    left_kind: str
    op: OperationName
    right_kind: str
    result_kind: str
    commutative: bool = False
    rationale: str | None = None

    def __post_init__(self) -> None:
        _require_non_empty_string(self.left_kind, "OperationRule.left_kind")
        _require_non_empty_string(self.right_kind, "OperationRule.right_kind")
        _require_non_empty_string(self.result_kind, "OperationRule.result_kind")


@dataclass(frozen=True)
class CheckResult:
    """Inspectable result from dimension-only or kind-aware validation."""

    ok: bool
    lhs_kind: str | None = None
    rhs_kind: str | None = None
    lhs_dimensions: Dimensions | None = None
    rhs_dimensions: Dimensions | None = None
    reason: str = ""
    steps: tuple[str, ...] = ()


def _native_error(exc: ValueError) -> Exception:
    """Python boundary: a native tagged tuple becomes a kind error, chained to it."""
    tag, *fields = exc.args
    if tag == "unknown" and fields[0] == "kind":
        return UnknownKindError(f"Unknown quantity kind: {fields[1]}")
    if tag == "duplicate" and fields[0] == "kind":
        return DuplicateKindError(f"Duplicate quantity kind: {fields[1]}")
    if tag == "duplicate_rule":
        left, op, right = fields
        return DuplicateOperationRuleError(f"Duplicate operation rule: {left} {op} {right}")
    if tag == "invalid_dimensions":
        left, op, right, result, signature, grade = fields
        return InvalidOperationRuleError(
            f"Operation rule is dimensionally invalid: {left} {op} {right} -> {result}; "
            f"derived {parse_dims_signature(signature)} at grade {grade}"
        )
    if tag == "derived_rule":
        left, op, right, result, derived = fields
        return DerivedOperationRuleError(
            f"Operation rule {left} {op} {right} -> {result} restates the derived kind {derived}"
        )
    if tag == "commutative_quotient":
        left, right = fields
        return InvalidOperationRuleError(f"division operation rules cannot be commutative: {left} div {right}")
    if tag == "ungraded_rule":
        left, op, right, left_grade, right_grade = fields
        return InvalidOperationRuleError(
            f"Operation rule {left} {op} {right} has no single grade (grades {left_grade} and {right_grade})"
        )
    if tag == "invalid_operation":
        (name,) = fields
        return InvalidOperationRuleError(f"Unsupported operation: {name}")
    if tag == "no_product_kind":
        left, op, right, signature, grade = fields
        return MissingOperationRuleError(
            f"No operation rule for {left} {op} {right}; "
            f"result dimensions {parse_dims_signature(signature)} at grade {grade}"
        )
    if tag == "unresolved_twin":
        left, op, right, twins = fields
        return MissingOperationRuleError(
            f"No operation rule for {left} {op} {right}; it could be any of {', '.join(twins)}"
        )
    if tag == "ungraded_product":
        left, op, right, left_grade, right_grade = fields
        return KindMismatchError(
            f"{left} {op} {right} has no single grade (grades {left_grade} and {right_grade})"
        )
    if tag == "unsupported_operation":
        operation, left, right = fields
        return KindMismatchError(
            f"{operation} is not defined for {left}" + ("" if right is None else f" and {right}")
        )
    if tag == "no_power_kind":
        base, exponent, signature, grade = fields
        return UnknownKindError(
            f"No quantity kind has dimensions {parse_dims_signature(signature)} "
            f"at grade {grade} for {base} pow {exponent}"
        )
    if tag == "unresolved_power_twin":
        base, exponent, twins = fields
        return AmbiguousKindError(f"{base} pow {exponent} could be any of: {', '.join(twins)}")
    if tag == "ungraded_power":
        base, exponent, grade = fields
        return KindMismatchError(f"{base} pow {exponent} has no single grade (grade {grade})")
    return exc


class KindRegistry:
    """A validated collection of quantity kinds and operation rules."""

    _native: NativeKindRegistry

    def __init__(
        self,
        *,
        kinds: Iterable[QuantityKind],
        rules: Iterable[OperationRule] = (),
    ) -> None:
        kinds = tuple(kinds)
        rules = tuple(rules)
        try:
            self._native = NativeKindRegistry(
                [{"name": kind.name, "dimensions": kind.dimensions} for kind in kinds],
                [
                    {
                        "left_kind": rule.left_kind,
                        "op": rule.op,
                        "right_kind": rule.right_kind,
                        "result_kind": rule.result_kind,
                        "commutative": rule.commutative,
                        "rationale": rule.rationale,
                    }
                    for rule in rules
                ],
            )
        except ValueError as exc:
            error = _native_error(exc)
            if error is exc:
                raise
            raise error from exc

    @classmethod
    def bundled(cls) -> KindRegistry:
        """The catalog Bridgman bundles (profiles/thermal.yml), as the Rust core compiles it."""
        registry = cls.__new__(cls)
        registry._native = NativeKindRegistry.bundled()
        return registry

    def kind_names(self) -> tuple[str, ...]:
        """Every kind, in declaration order."""
        return tuple(self._native.kinds())

    def kind_dimensions(self, kind_name: str) -> Dimensions:
        """Return a copy of a kind's canonical dimensions."""
        try:
            return self._native.kind_dimensions(kind_name)
        except ValueError as exc:
            raise UnknownKindError(f"Unknown quantity kind: {kind_name}") from exc

    def result_kind(self, left_kind: str, op: OperationName, right_kind: str) -> str:
        """The kind of `left op right`, derived by the Rust core; a rule chooses only between twins."""
        try:
            return self._native.result_kind(left_kind, op, right_kind)
        except ValueError as exc:
            error = _native_error(exc)
            if error is exc:
                raise
            raise error from exc

    def power_kind(self, base_kind: str, exponent: int) -> str:
        """The kind of `base ** exponent`, derived by the Rust core."""
        try:
            return self._native.power_kind(base_kind, exponent)
        except ValueError as exc:
            error = _native_error(exc)
            if error is exc:
                raise
            raise error from exc

    def rule_rationale(self, left_kind: str, op: OperationName, right_kind: str) -> str | None:
        """The rationale of the rule that chose `left op right` between twins, if any."""
        try:
            return self._native.row_provenance(left_kind, op, right_kind)
        except ValueError as exc:
            error = _native_error(exc)
            if error is exc:
                raise
            raise error from exc

    def kinds_with_dimensions(self, dimensions: Dimensions) -> tuple[str, ...]:
        """Return all kind names whose dimensions match the supplied dimensions."""
        return tuple(self._native.kinds_with_dimensions(dimensions))

    def ambiguous_kinds(self, dimensions: Dimensions) -> tuple[str, ...]:
        """Return matching kind names only when dimensions identify multiple kinds."""
        matches = self.kinds_with_dimensions(dimensions)
        return matches if len(matches) > 1 else ()


__all__ = [
    "AmbiguousKindError",
    "CheckResult",
    "DerivedOperationRuleError",
    "DuplicateKindError",
    "DuplicateOperationRuleError",
    "InvalidOperationRuleError",
    "KindError",
    "KindMismatchError",
    "KindRegistry",
    "MissingOperationRuleError",
    "OperationName",
    "OperationRule",
    "QuantityKind",
    "UnknownKindError",
]
