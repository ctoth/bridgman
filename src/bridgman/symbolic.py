"""Dimensional and kind analysis of sympy expression trees.

This module only walks sympy trees. Every rule it applies is the Rust core's:
dimension arithmetic (including rational exponents) through the dimension
functions, and kind arithmetic through `KindRegistry`, which asks `Kind::combine`,
`Kind::power`, `Kind::scaled` and `Kind::same`. A pure number has no kind.
"""

from __future__ import annotations

from dataclasses import dataclass
from fractions import Fraction

from sympy import (
    Abs,
    Add,
    Max,
    Min,
    cos,
    cosh,
    exp,
    Float,
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

from bridgman._core import BridgmanError
from bridgman.dimensions import (
    Dimensions,
    canonicalize_dims,
    dims_equal,
    is_dimensionless,
    mul_dims,
    pow_dims,
)
from bridgman.kinds import CheckResult, KindRegistry


class DimensionalError(Exception):
    """Raised when an expression's terms cannot agree (e.g. adding m + v), or
    when a sympy construct has no dimensional reading."""


@dataclass(frozen=True)
class _KindDetails:
    kind: str | None
    dimensions: Dimensions
    steps: tuple[str, ...] = ()


_DIMENSIONLESS_ARG_FUNCTIONS = {
    sin,
    cos,
    tan,
    exp,
    log,
    sinh,
    cosh,
    tanh,
}


def _dims_of_same_dimension_args(
    expr,
    dim_map: dict[str, Dimensions],
    context: str,
) -> Dimensions:
    dims_list = [dims_of_expr(arg, dim_map) for arg in expr.args]
    if not dims_list:
        return {}

    first = dims_list[0]
    for i, dims in enumerate(dims_list[1:], 1):
        if not dims_equal(first, dims):
            raise DimensionalError(
                f"Dimensional mismatch in {context}: "
                f"argument 0 has {first}, argument {i} has {dims}"
            )
    return first


def _exponent(exponent) -> Fraction:
    """A sympy exponent as an exact Fraction; the core's exponents are exact."""
    if isinstance(exponent, Rational):
        return Fraction(exponent.p, exponent.q)
    if isinstance(exponent, Float):
        raise DimensionalError(
            f"floating exponent in power expression is not dimensionally exact: {exponent}"
        )
    raise DimensionalError(f"non-numeric exponent in power expression: {exponent}")


def _dims_of_dimensionless_arg_function(expr, dim_map: dict[str, Dimensions]) -> Dimensions:
    for arg in expr.args:
        arg_dims = dims_of_expr(arg, dim_map)
        if not is_dimensionless(arg_dims):
            raise DimensionalError(
                f"{expr.func.__name__} argument must be dimensionless; got {arg_dims}"
            )
    return {}


def dims_of_expr(expr, dim_map: dict[str, Dimensions]) -> Dimensions:
    """Compute the dimensions of a sympy expression.

    Args:
        expr: A sympy expression.
        dim_map: Maps symbol names (strings) to their Dimensions dicts.

    Returns:
        The resulting Dimensions dict; a root gives Fraction exponents.

    Raises:
        KeyError: If a symbol is not found in dim_map.
        DimensionalError: If dimensions are inconsistent (e.g. in addition).
    """
    if isinstance(expr, Symbol):
        name = expr.name
        if name not in dim_map:
            raise KeyError(name)
        return canonicalize_dims(dim_map[name])

    if isinstance(expr, (Number, NumberSymbol)):
        # Numeric constants (Integer, Rational, Float, pi, etc.) are dimensionless
        return {}

    if isinstance(expr, Mul):
        result: Dimensions = {}
        for arg in expr.args:
            result = mul_dims(result, dims_of_expr(arg, dim_map))
        return result

    if isinstance(expr, Pow):
        base_dims = dims_of_expr(expr.args[0], dim_map)
        if not isinstance(expr.args[1], Rational) and is_dimensionless(base_dims):
            # The core's exponents are exact numbers. A symbolic or floating
            # exponent is readable only on a dimensionless base, which any
            # power leaves dimensionless.
            return {}
        return pow_dims(base_dims, _exponent(expr.args[1]))

    if isinstance(expr, Add):
        return _dims_of_same_dimension_args(expr, dim_map, "addition")

    if isinstance(expr, Relational):
        raise DimensionalError("Nested relational expressions are not dimension terms")

    if getattr(expr, "func", None) in _DIMENSIONLESS_ARG_FUNCTIONS:
        return _dims_of_dimensionless_arg_function(expr, dim_map)

    if getattr(expr, "func", None) == atan2:
        _dims_of_same_dimension_args(expr, dim_map, "atan2")
        return {}

    if getattr(expr, "func", None) == Abs:
        return dims_of_expr(expr.args[0], dim_map)

    if getattr(expr, "func", None) in {Min, Max}:
        return _dims_of_same_dimension_args(expr, dim_map, expr.func.__name__)

    raise DimensionalError(f"Unsupported sympy expression type: {type(expr).__name__}")


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


def _kind_details_of_symbol(
    expr: Symbol,
    registry: KindRegistry,
    kind_map: dict[str, str],
) -> _KindDetails:
    name = expr.name
    if name not in kind_map:
        raise KeyError(name)

    kind = kind_map[name]
    return _details(kind, registry, (f"symbol {name} -> {kind}",))


def _product(
    left: _KindDetails,
    divide: bool,
    right: _KindDetails,
    registry: KindRegistry,
) -> _KindDetails:
    """`left * right` or `left / right`, where either side may be a pure number."""
    steps = left.steps + right.steps
    op = "div" if divide else "mul"
    if right.kind is None:
        if left.kind is None:
            return _KindDetails(None, {}, steps)
        scaled = registry.divided_kind(left.kind) if divide else registry.scaled_kind(left.kind)
        return _details(scaled, registry, steps)
    if left.kind is None:
        kind = registry.power_kind(right.kind, -1) if divide else registry.scaled_kind(right.kind)
        return _details(kind, registry, steps + ((f"{right.kind} pow -1 -> {kind}",) if divide else ()))

    result_kind = registry.result_kind(left.kind, op, right.kind)
    rationale = registry.rule_rationale(left.kind, op, right.kind)
    step = f"{left.kind} {op} {right.kind} -> {result_kind}"
    if rationale is not None:
        step = f"{step}; {rationale}"
    return _details(result_kind, registry, steps + (step,))


def _is_reciprocal(expr) -> bool:
    return isinstance(expr, Pow) and expr.args[1] == Integer(-1)


def _terms_of_one_kind(
    expr,
    registry: KindRegistry,
    kind_map: dict[str, str],
    context: str,
    combine,
) -> _KindDetails:
    """Terms that must share a kind: a sum (`Kind::combine` with add) or a
    comparison (`Kind::same`). A pure number shares no kind with a quantity."""
    details = [_kind_details_of_expr(arg, registry, kind_map) for arg in expr.args]
    if not details:
        return _KindDetails(None, {})

    result = details[0]
    for i, current in enumerate(details[1:], 1):
        steps = result.steps + current.steps
        if result.kind is None and current.kind is None:
            result = _KindDetails(None, {}, steps)
        elif result.kind is None or current.kind is None:
            raise DimensionalError(
                f"{context}: argument 0 has kind {result.kind}, argument {i} has kind {current.kind}; "
                "a pure number has no kind"
            )
        else:
            result = _details(combine(result.kind, current.kind), registry, steps)
    return result


def _dimensionless_function_kind_details(
    expr,
    registry: KindRegistry,
    kind_map: dict[str, str],
) -> _KindDetails:
    steps: tuple[str, ...] = ()
    for arg in expr.args:
        arg_details = _kind_details_of_expr(arg, registry, kind_map)
        if not is_dimensionless(arg_details.dimensions):
            raise DimensionalError(
                f"{expr.func.__name__} argument must be dimensionless; "
                f"got {arg_details.dimensions}"
            )
        steps += arg_details.steps
    return _KindDetails(None, {}, steps)


def _kind_details_of_expr(
    expr,
    registry: KindRegistry,
    kind_map: dict[str, str],
) -> _KindDetails:
    if isinstance(expr, Symbol):
        return _kind_details_of_symbol(expr, registry, kind_map)

    if isinstance(expr, (Number, NumberSymbol)):
        return _KindDetails(None, {})

    if isinstance(expr, Add):
        return _terms_of_one_kind(
            expr,
            registry,
            kind_map,
            "addition",
            lambda left, right: registry.result_kind(left, "add", right),
        )

    if isinstance(expr, Mul):
        result = _KindDetails(None, {})
        for arg in expr.args:
            if _is_reciprocal(arg):
                right = _kind_details_of_expr(arg.args[0], registry, kind_map)
                result = _product(result, True, right, registry)
            else:
                right = _kind_details_of_expr(arg, registry, kind_map)
                result = _product(result, False, right, registry)
        return result

    if isinstance(expr, Pow):
        base = _kind_details_of_expr(expr.args[0], registry, kind_map)
        if base.kind is None:
            return _KindDetails(None, {}, base.steps)
        exponent = _exponent(expr.args[1])
        result_kind = registry.power_kind(base.kind, exponent)
        return _details(
            result_kind,
            registry,
            base.steps + (f"{base.kind} pow {exponent} -> {result_kind}",),
        )

    if isinstance(expr, Relational):
        raise DimensionalError("Nested relational expressions are not kind terms")

    if getattr(expr, "func", None) in _DIMENSIONLESS_ARG_FUNCTIONS:
        return _dimensionless_function_kind_details(expr, registry, kind_map)

    if getattr(expr, "func", None) == atan2:
        details = [_kind_details_of_expr(arg, registry, kind_map) for arg in expr.args]
        if not dims_equal(details[0].dimensions, details[1].dimensions):
            raise DimensionalError(
                f"Dimensional mismatch in atan2: argument 0 has {details[0].dimensions}, "
                f"argument 1 has {details[1].dimensions}"
            )
        return _KindDetails(None, {}, details[0].steps + details[1].steps)

    if getattr(expr, "func", None) == Abs:
        base = _kind_details_of_expr(expr.args[0], registry, kind_map)
        if base.kind is None:
            return base
        return _details(registry.absolute_kind(base.kind), registry, base.steps)

    if getattr(expr, "func", None) in {Min, Max}:
        return _terms_of_one_kind(
            expr, registry, kind_map, expr.func.__name__, registry.same_kind
        )

    raise DimensionalError(f"Unsupported sympy expression type: {type(expr).__name__}")


def kind_of_expr(expr, *, registry: KindRegistry, kind_map: dict[str, str]) -> str | None:
    """Infer the semantic kind of a sympy expression."""
    return _kind_details_of_expr(expr, registry, kind_map).kind


def verify_expr_kinds(eq, *, registry: KindRegistry, kind_map: dict[str, str]) -> bool:
    """Verify that both sides of a sympy relation have the same semantic kind."""
    if not isinstance(eq, Relational):
        raise TypeError(f"Expected sympy relational expression, got {type(eq).__name__}")

    lhs = _kind_details_of_expr(eq.args[0], registry, kind_map)
    rhs = _kind_details_of_expr(eq.args[1], registry, kind_map)
    return lhs.kind == rhs.kind


def explain_expr(eq, dim_map: dict[str, Dimensions]) -> CheckResult:
    """Return an inspectable dimension-only validation result."""
    if not isinstance(eq, Relational):
        raise TypeError(f"Expected sympy relational expression, got {type(eq).__name__}")

    try:
        lhs_dims = dims_of_expr(eq.args[0], dim_map)
        rhs_dims = dims_of_expr(eq.args[1], dim_map)
    except Exception as exc:
        return CheckResult(False, reason=str(exc), steps=(str(exc),))

    if dims_equal(lhs_dims, rhs_dims):
        return CheckResult(
            True,
            lhs_dimensions=lhs_dims,
            rhs_dimensions=rhs_dims,
            reason="matching dimensions",
            steps=(f"lhs dimensions {lhs_dims}", f"rhs dimensions {rhs_dims}"),
        )

    return CheckResult(
        False,
        lhs_dimensions=lhs_dims,
        rhs_dimensions=rhs_dims,
        reason=f"dimension mismatch: lhs {lhs_dims}, rhs {rhs_dims}",
        steps=(f"lhs dimensions {lhs_dims}", f"rhs dimensions {rhs_dims}"),
    )


def _refusal(exc: Exception) -> str:
    """A refusal as reason text, naming the class that raised it
    (`QuantityError.NoProductKind: ...`)."""
    return f"{type(exc).__qualname__}: {exc}"


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
    except (BridgmanError, DimensionalError, KeyError) as exc:
        reason = _refusal(exc)
        return CheckResult(
            False,
            lhs_kind=lhs.kind if lhs else None,
            lhs_dimensions=lhs.dimensions if lhs else None,
            reason=reason,
            steps=(reason,) if lhs is None else lhs.steps + (reason,),
        )

    steps = lhs.steps + rhs.steps
    if lhs.kind == rhs.kind:
        return CheckResult(
            True,
            lhs_kind=lhs.kind,
            rhs_kind=rhs.kind,
            lhs_dimensions=lhs.dimensions,
            rhs_dimensions=rhs.dimensions,
            reason="same kind and dimensions",
            steps=steps,
        )

    return CheckResult(
        False,
        lhs_kind=lhs.kind,
        rhs_kind=rhs.kind,
        lhs_dimensions=lhs.dimensions,
        rhs_dimensions=rhs.dimensions,
        reason=(
            f"kind mismatch: lhs {lhs.kind} {lhs.dimensions}, "
            f"rhs {rhs.kind} {rhs.dimensions}"
        ),
        steps=steps,
    )
