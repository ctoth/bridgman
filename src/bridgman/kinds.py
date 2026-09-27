"""Semantic quantity kinds layered over dimension arithmetic.

Every rule lives in the Rust core. A registry is a catalog the core reads
through its own schema, and every refusal is the core's error, raised as the
class of its Rust variant (`bridgman.QuantityError.NoProductKind`, and so on).
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from fractions import Fraction
from typing import Iterable, Literal

from bridgman._core import CATALOG_SCHEMA, NativeKindRegistry

from bridgman.dimensions import Dimensions, canonicalize_dims


OperationName = Literal["add", "sub", "mul", "div", "dot", "wedge"]
ProductName = Literal["mul", "div", "dot", "wedge"]


@dataclass(frozen=True)
class QuantityKind:
    """A semantic quantity kind with dimensions."""

    name: str
    dimensions: Dimensions

    def __post_init__(self) -> None:
        object.__setattr__(self, "dimensions", canonicalize_dims(self.dimensions))


@dataclass(frozen=True)
class OperationRule:
    """A declared row choosing which twin `left op right` is."""

    left_kind: str
    op: ProductName
    right_kind: str
    result_kind: str
    commutative: bool = False
    rationale: str | None = None


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


def _catalog(kinds: Iterable[QuantityKind], rules: Iterable[OperationRule]) -> str:
    """The catalog document these declarations are, in the core's schema.
    Exponents are written as text, the schema's exact form for any rational."""
    return json.dumps(
        {
            "schema": CATALOG_SCHEMA,
            "kinds": [
                {
                    "id": kind.name,
                    "dimensions": {base: str(power) for base, power in kind.dimensions.items()},
                }
                for kind in kinds
            ],
            "units": [],
            "operations": [
                {
                    "left": rule.left_kind,
                    "op": rule.op,
                    "right": rule.right_kind,
                    "result": rule.result_kind,
                    "commutative": rule.commutative,
                    "provenance": rule.rationale,
                }
                for rule in rules
            ],
        }
    )


class KindRegistry:
    """A compiled catalog of quantity kinds; every answer is the Rust core's."""

    _native: NativeKindRegistry

    def __init__(
        self,
        *,
        kinds: Iterable[QuantityKind],
        rules: Iterable[OperationRule] = (),
    ) -> None:
        self._native = NativeKindRegistry(_catalog(kinds, rules))

    @classmethod
    def bundled(cls) -> KindRegistry:
        """The catalog Bridgman bundles (catalogs/thermal.yml), as the Rust core compiles it."""
        registry = cls.__new__(cls)
        registry._native = NativeKindRegistry.bundled()
        return registry

    def kind_names(self) -> tuple[str, ...]:
        """Every kind, in declaration order."""
        return tuple(self._native.kinds())

    def kind_dimensions(self, kind_name: str) -> Dimensions:
        """A kind's canonical dimensions."""
        return self._native.kind_dimensions(kind_name)

    def result_kind(self, left_kind: str, op: OperationName, right_kind: str) -> str:
        """The kind of `left op right` (`Kind::combine`); a rule chooses only between twins."""
        return self._native.result_kind(left_kind, op, right_kind)

    def power_kind(self, base_kind: str, exponent: int | Fraction) -> str:
        """The kind of `base ** exponent` (`Kind::power`); a root is a fractional power."""
        return self._native.power_kind(base_kind, exponent)

    def scaled_kind(self, kind_name: str) -> str:
        """The kind of a value of this kind times a pure number (`Kind::scaled`)."""
        return self._native.scaled_kind(kind_name)

    def divided_kind(self, kind_name: str) -> str:
        """The kind of a value of this kind divided by a pure number (`Kind::scaled`)."""
        return self._native.divided_kind(kind_name)

    def absolute_kind(self, kind_name: str) -> str:
        """The kind of a value of this kind with its sign dropped (`Kind::scaled`)."""
        return self._native.absolute_kind(kind_name)

    def same_kind(self, left_kind: str, right_kind: str) -> str:
        """The one kind two compared values share (`Kind::same`)."""
        return self._native.same_kind(left_kind, right_kind)

    def rule_rationale(self, left_kind: str, op: ProductName, right_kind: str) -> str | None:
        """The rationale of the rule that chose `left op right` between twins, if any."""
        return self._native.row_provenance(left_kind, op, right_kind)

    def kinds_with_dimensions(self, dimensions: Dimensions) -> tuple[str, ...]:
        """Return all kind names whose dimensions match the supplied dimensions."""
        return tuple(self._native.kinds_with_dimensions(dimensions))

    def ambiguous_kinds(self, dimensions: Dimensions) -> tuple[str, ...]:
        """Return matching kind names only when dimensions identify multiple kinds."""
        matches = self.kinds_with_dimensions(dimensions)
        return matches if len(matches) > 1 else ()


__all__ = [
    "CheckResult",
    "KindRegistry",
    "OperationName",
    "OperationRule",
    "ProductName",
    "QuantityKind",
]
