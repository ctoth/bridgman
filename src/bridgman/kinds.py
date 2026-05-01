"""Semantic quantity kinds layered over dimension arithmetic."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, Literal

from bridgman.dimensions import Dimensions, canonicalize_dims, dims_equal, div_dims, mul_dims


OperationName = Literal["mul", "div"]


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
        if self.op not in {"mul", "div"}:
            raise InvalidOperationRuleError(f"Unsupported operation: {self.op}")
        if self.op == "div" and self.commutative:
            raise InvalidOperationRuleError("division operation rules cannot be commutative")


class KindRegistry:
    """A validated collection of quantity kinds and operation rules."""

    def __init__(
        self,
        *,
        kinds: Iterable[QuantityKind],
        rules: Iterable[OperationRule] = (),
    ) -> None:
        self._kinds: dict[str, QuantityKind] = {}
        for kind in kinds:
            if kind.name in self._kinds:
                raise DuplicateKindError(f"Duplicate quantity kind: {kind.name}")
            self._kinds[kind.name] = kind

        self._rules: dict[tuple[str, OperationName, str], OperationRule] = {}
        for rule in rules:
            self._add_rule(rule, rule.left_kind, rule.right_kind)
            if rule.commutative and rule.left_kind != rule.right_kind:
                self._add_rule(rule, rule.right_kind, rule.left_kind)

    def _add_rule(self, rule: OperationRule, left_kind: str, right_kind: str) -> None:
        self._validate_rule(rule)
        key = (left_kind, rule.op, right_kind)
        if key in self._rules:
            raise DuplicateOperationRuleError(
                f"Duplicate operation rule: {left_kind} {rule.op} {right_kind}"
            )
        self._rules[key] = rule

    def _validate_rule(self, rule: OperationRule) -> None:
        for kind_name in (rule.left_kind, rule.right_kind, rule.result_kind):
            if kind_name not in self._kinds:
                raise UnknownKindError(f"Unknown quantity kind: {kind_name}")

        left_dims = self._kinds[rule.left_kind].dimensions
        right_dims = self._kinds[rule.right_kind].dimensions
        result_dims = self._kinds[rule.result_kind].dimensions
        expected_dims = (
            mul_dims(left_dims, right_dims)
            if rule.op == "mul"
            else div_dims(left_dims, right_dims)
        )
        if not dims_equal(expected_dims, result_dims):
            raise InvalidOperationRuleError(
                f"Operation rule is dimensionally invalid: "
                f"{rule.left_kind} {rule.op} {rule.right_kind} -> {rule.result_kind}; "
                f"expected {expected_dims}, got {result_dims}"
            )

    def kind_dimensions(self, kind_name: str) -> Dimensions:
        """Return a copy of a kind's canonical dimensions."""
        try:
            return dict(self._kinds[kind_name].dimensions)
        except KeyError as exc:
            raise UnknownKindError(f"Unknown quantity kind: {kind_name}") from exc

    def result_kind(self, left_kind: str, op: OperationName, right_kind: str) -> str:
        """Return the declared result kind for an operation."""
        return self.operation_rule(left_kind, op, right_kind).result_kind

    def operation_rule(
        self,
        left_kind: str,
        op: OperationName,
        right_kind: str,
    ) -> OperationRule:
        """Return the declared operation rule for an operation."""
        key = (left_kind, op, right_kind)
        try:
            return self._rules[key]
        except KeyError as exc:
            raise MissingOperationRuleError(f"No operation rule for {left_kind} {op} {right_kind}") from exc

    def kinds_with_dimensions(self, dimensions: Dimensions) -> tuple[str, ...]:
        """Return all kind names whose dimensions match the supplied dimensions."""
        target = canonicalize_dims(dimensions)
        return tuple(
            kind.name for kind in self._kinds.values() if dims_equal(kind.dimensions, target)
        )

    def unique_kind_with_dimensions(self, dimensions: Dimensions) -> str:
        """Return the only kind matching dimensions, or fail if none or many match."""
        matches = self.kinds_with_dimensions(dimensions)
        if not matches:
            raise UnknownKindError(f"No quantity kind has dimensions: {dimensions}")
        if len(matches) == 1:
            return next(iter(matches))
        if len(matches) > 1:
            raise AmbiguousKindError(
                f"Dimensions {dimensions} match multiple quantity kinds: {', '.join(matches)}"
            )
        raise UnknownKindError(f"No quantity kind has dimensions: {dimensions}")

    def ambiguous_kinds(self, dimensions: Dimensions) -> tuple[str, ...]:
        """Return matching kind names only when dimensions identify multiple kinds."""
        matches = self.kinds_with_dimensions(dimensions)
        return matches if len(matches) > 1 else ()


__all__ = [
    "AmbiguousKindError",
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
