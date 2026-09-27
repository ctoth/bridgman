"""Dimensional and kind analysis of sympy expression trees.

This module walks sympy trees and asks the Rust core every question: the
dimensions of a product, power, sum or transcendental (`Dimensions::raised`,
`common`, `transcendental`), and the kind of a term (`Term::combine`, `power`,
`absolute`, `same`, where `None` is a pure number). Each refusal is the
core's, raised as its Rust variant. The only error of its own is
`UnsupportedExpressionError`, for a sympy construct it cannot walk.
"""

from __future__ import annotations

from dataclasses import dataclass
from fractions import Fraction
from typing import Literal

from sympy import (
    Abs,
    Add,
    Max,
    Min,
    cos,
    cosh,
    exp,
    Integer,
    log,
    Mul,
    Number,
    NumberSymbol,
    Pow,
    Rational,
    sin,
    sinh,
    Symbol,
    tan,
    tanh,
    atan2,
)
from sympy.core.relational import Relational

from bridgman._core import BridgmanError, QuantityError
from bridgman.dimensions import (
    Dimensions,
    canonicalize_dims,
    common_dims,
    dims_equal,
    div_dims,
    mul_dims,
    pow_dims,
    transcendental_dims,
)
from bridgman.kinds import CheckResult, KindRegistry


class UnsupportedExpressionError(TypeError):
    """Raised for a sympy construct the walker cannot read, such as a
    derivative or a nested relation. It judges nothing about dimensions."""


@dataclass(frozen=True)
class _KindDetails:
    kind: str | None
    dimensions: Dimensions
    steps: tuple[str, ...] = ()


_TRANSCENDENTAL_FUNCTIONS = {
    sin,
    cos,
    tan,
    exp,
    log,
    sinh,
    cosh,
    tanh,
}


def _exponent(exponent) -> Fraction | None:
    """A sympy exponent as the core reads it: an exact Fraction, or None for
    one known only inexactly (a symbol or a float)."""
    if isinstance(exponent, Rational):
        return Fraction(exponent.p, exponent.q)
    return None


def _unsupported(expr) -> UnsupportedExpressionError:
    if isinstance(expr, Relational):
        return UnsupportedExpressionError("Nested relational expressions are not terms")
    return UnsupportedExpressionError(f"Unsupported sympy expression type: {type(expr).__name__}")


def _common(dimensions: list[Dimensions]) -> Dimensions:
    result = dimensions[0]
    for other in dimensions[1:]:
        result = common_dims(result, other)
    return result


def dims_of_expr(expr, dim_map: dict[str, Dimensions]) -> Dimensions:
    """Compute the dimensions of a sympy expression.

    Args:
        expr: A sympy expression.
        dim_map: Maps symbol names (strings) to their Dimensions dicts.

    Returns:
        The resulting Dimensions dict; a root gives Fraction exponents.

    Raises:
        KeyError: If a symbol is not found in dim_map.
        DimensionError: The core's refusal (unequal terms, a dimensioned
            transcendental argument, an inexact exponent on a dimensioned base).
        UnsupportedExpressionError: For a construct the walker cannot read.
    """
    if isinstance(expr, Symbol):
        name = expr.name
        if name not in dim_map:
            raise KeyError(name)
        return canonicalize_dims(dim_map[name])

    if isinstance(expr, (Number, NumberSymbol)):
        return {}

    if isinstance(expr, Mul):
        result: Dimensions = {}
        for arg in expr.args:
            result = mul_dims(result, dims_of_expr(arg, dim_map))
        return result

    if isinstance(expr, Pow):
        return pow_dims(dims_of_expr(expr.args[0], dim_map), _exponent(expr.args[1]))

    if isinstance(expr, Add) or getattr(expr, "func", None) in {Min, Max}:
        return _common([dims_of_expr(arg, dim_map) for arg in expr.args])

    if getattr(expr, "func", None) in _TRANSCENDENTAL_FUNCTIONS:
        return transcendental_dims(dims_of_expr(expr.args[0], dim_map))

    if getattr(expr, "func", None) == atan2:
        y, x = (dims_of_expr(arg, dim_map) for arg in expr.args)
        return transcendental_dims(div_dims(y, x))

    if getattr(expr, "func", None) == Abs:
        return dims_of_expr(expr.args[0], dim_map)

    raise _unsupported(expr)


def verify_expr(eq, dim_map: dict[str, Dimensions]) -> bool:
    """Verify that both sides of a sympy relation have the same dimensions.

    Args:
        eq: A sympy Eq or inequality expression.
        dim_map: Maps symbol names (strings) to their Dimensions dicts.

    Returns:
        True if both sides have matching dimensions, False otherwise.
    """
    if not isinstance(eq, Relational):
        raise TypeError(f"Expected sympy relational expression, got {type(eq).__name__}")

    lhs_dims = dims_of_expr(eq.args[0], dim_map)
    rhs_dims = dims_of_expr(eq.args[1], dim_map)
    return dims_equal(lhs_dims, rhs_dims)


def _details(kind: str | None, registry: KindRegistry, steps: tuple[str, ...]) -> _KindDetails:
    dimensions: Dimensions = {} if kind is None else registry.kind_dimensions(kind)
    return _KindDetails(kind, dimensions, steps)


def _name(kind: str | None) -> str:
    return "number" if kind is None else kind


def _combine(
    left: _KindDetails,
    op: Literal["add", "mul", "div"],
    right: _KindDetails,
    registry: KindRegistry,
) -> _KindDetails:
    """`left op right` by `Term::combine`, with the step it took."""
    result: str | None = registry.result_kind(left.kind, op, right.kind)
    steps = left.steps + right.steps
    if left.kind is None and right.kind is None:
        return _details(result, registry, steps)
    step = f"{_name(left.kind)} {op} {_name(right.kind)} -> {_name(result)}"
    if left.kind is not None and right.kind is not None and op != "add":
        rationale = registry.rule_rationale(left.kind, op, right.kind)
        if rationale is not None:
            step = f"{step}; {rationale}"
    return _details(result, registry, steps + (step,))


def _is_reciprocal(expr) -> bool:
    return isinstance(expr, Pow) and expr.args[1] == Integer(-1)


def _kind_details_of_expr(
    expr,
    registry: KindRegistry,
    kind_map: dict[str, str],
) -> _KindDetails:
    if isinstance(expr, Symbol):
        if expr.name not in kind_map:
            raise KeyError(expr.name)
        kind = kind_map[expr.name]
        return _details(kind, registry, (f"symbol {expr.name} -> {kind}",))

    if isinstance(expr, (Number, NumberSymbol)):
        return _KindDetails(None, {})

    if isinstance(expr, Add):
        details = [_kind_details_of_expr(arg, registry, kind_map) for arg in expr.args]
        result = details[0]
        for current in details[1:]:
            result = _combine(result, "add", current, registry)
        return result

    if isinstance(expr, Mul):
        result = _KindDetails(None, {})
        for arg in expr.args:
            if _is_reciprocal(arg):
                right = _kind_details_of_expr(arg.args[0], registry, kind_map)
                result = _combine(result, "div", right, registry)
            else:
                right = _kind_details_of_expr(arg, registry, kind_map)
                result = _combine(result, "mul", right, registry)
        return result

    if isinstance(expr, Pow):
        base = _kind_details_of_expr(expr.args[0], registry, kind_map)
        exponent = _exponent(expr.args[1])
        result_kind = registry.power_kind(base.kind, exponent)
        steps = base.steps
        if base.kind is not None:
            steps += (f"{base.kind} pow {exponent} -> {_name(result_kind)}",)
        return _details(result_kind, registry, steps)

    if getattr(expr, "func", None) in _TRANSCENDENTAL_FUNCTIONS:
        argument = _kind_details_of_expr(expr.args[0], registry, kind_map)
        transcendental_dims(argument.dimensions)
        return _KindDetails(None, {}, argument.steps)

    if getattr(expr, "func", None) == atan2:
        y, x = (_kind_details_of_expr(arg, registry, kind_map) for arg in expr.args)
        transcendental_dims(div_dims(y.dimensions, x.dimensions))
        return _KindDetails(None, {}, y.steps + x.steps)

    if getattr(expr, "func", None) == Abs:
        base = _kind_details_of_expr(expr.args[0], registry, kind_map)
        return _details(registry.absolute_kind(base.kind), registry, base.steps)

    if getattr(expr, "func", None) in {Min, Max}:
        details = [_kind_details_of_expr(arg, registry, kind_map) for arg in expr.args]
        result = details[0]
        for current in details[1:]:
            result = _details(
                registry.same_kind(result.kind, current.kind),
                registry,
                result.steps + current.steps,
            )
        return result

    raise _unsupported(expr)


def kind_of_expr(expr, *, registry: KindRegistry, kind_map: dict[str, str]) -> str | None:
    """Infer the semantic kind of a sympy expression; None is a pure number."""
    return _kind_details_of_expr(expr, registry, kind_map).kind


def verify_expr_kinds(eq, *, registry: KindRegistry, kind_map: dict[str, str]) -> bool:
    """Verify that both sides of a sympy relation are the same kind (`Term::same`)."""
    if not isinstance(eq, Relational):
        raise TypeError(f"Expected sympy relational expression, got {type(eq).__name__}")

    lhs = _kind_details_of_expr(eq.args[0], registry, kind_map)
    rhs = _kind_details_of_expr(eq.args[1], registry, kind_map)
    try:
        registry.same_kind(lhs.kind, rhs.kind)
    except (QuantityError.KindMismatch, QuantityError.NumberTerm):
        return False
    return True


def _refusal(exc: Exception) -> str:
    """A refusal as reason text, naming the class that raised it
    (`QuantityError.NoProductKind: ...`)."""
    return f"{type(exc).__qualname__}: {exc}"


def explain_expr(eq, dim_map: dict[str, Dimensions]) -> CheckResult:
    """Return an inspectable dimension-only validation result."""
    if not isinstance(eq, Relational):
        raise TypeError(f"Expected sympy relational expression, got {type(eq).__name__}")

    try:
        lhs_dims = dims_of_expr(eq.args[0], dim_map)
        rhs_dims = dims_of_expr(eq.args[1], dim_map)
    except (BridgmanError, UnsupportedExpressionError, KeyError) as exc:
        reason = _refusal(exc)
        return CheckResult(False, reason=reason, steps=(reason,))

    steps = (f"lhs dimensions {lhs_dims}", f"rhs dimensions {rhs_dims}")
    try:
        common_dims(lhs_dims, rhs_dims)
    except BridgmanError as exc:
        return CheckResult(
            False,
            lhs_dimensions=lhs_dims,
            rhs_dimensions=rhs_dims,
            reason=_refusal(exc),
            steps=steps,
        )
    return CheckResult(
        True,
        lhs_dimensions=lhs_dims,
        rhs_dimensions=rhs_dims,
        reason="matching dimensions",
        steps=steps,
    )


def explain_expr_kinds(
    eq,
    *,
    registry: KindRegistry,
    kind_map: dict[str, str],
) -> CheckResult:
    """Return an inspectable kind-aware validation result."""
    if not isinstance(eq, Relational):
        raise TypeError(f"Expected sympy relational expression, got {type(eq).__name__}")

    lhs: _KindDetails | None = None
    try:
        lhs = _kind_details_of_expr(eq.args[0], registry, kind_map)
        rhs = _kind_details_of_expr(eq.args[1], registry, kind_map)
    except (BridgmanError, UnsupportedExpressionError, KeyError) as exc:
        reason = _refusal(exc)
        return CheckResult(
            False,
            lhs_kind=lhs.kind if lhs else None,
            lhs_dimensions=lhs.dimensions if lhs else None,
            reason=reason,
            steps=(reason,) if lhs is None else lhs.steps + (reason,),
        )

    steps = lhs.steps + rhs.steps
    try:
        registry.same_kind(lhs.kind, rhs.kind)
    except BridgmanError as exc:
        return CheckResult(
            False,
            lhs_kind=lhs.kind,
            rhs_kind=rhs.kind,
            lhs_dimensions=lhs.dimensions,
            rhs_dimensions=rhs.dimensions,
            reason=_refusal(exc),
            steps=steps,
        )
    return CheckResult(
        True,
        lhs_kind=lhs.kind,
        rhs_kind=rhs.kind,
        lhs_dimensions=lhs.dimensions,
        rhs_dimensions=rhs.dimensions,
        reason="same kind and dimensions",
        steps=steps,
    )
