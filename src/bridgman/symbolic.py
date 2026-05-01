"""Dimensional analysis of sympy expression trees."""

from dataclasses import dataclass
from fractions import Fraction

from sympy import (
    Abs,
    Add,
    Max,
    Min,
    cos,
    cosh,
    Eq,
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

from bridgman.dimensions import Dimensions, _clean, dims_equal, is_dimensionless, mul_dims, pow_dims
from bridgman.kinds import (
    AmbiguousKindError,
    CheckResult,
    KindMismatchError,
    KindRegistry,
    MissingOperationRuleError,
    OperationName,
    UnknownKindError,
)


class DimensionalError(Exception):
    """Raised when dimensions are inconsistent (e.g. adding m + v)."""


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


def _pow_dims_frac(d: Dimensions, exp: Fraction) -> Dimensions:
    """Raise dimensions to a fractional power.

    Multiplies each exponent by exp. If any result is not an integer,
    raises DimensionalError.
    """
    result: dict[str, int] = {}
    for k, v in d.items():
        new_exp = Fraction(v) * exp
        if new_exp.denominator != 1:
            raise DimensionalError(
                f"Fractional dimension exponent: {k}^{new_exp} "
                f"(from {k}^{v} raised to power {exp})"
            )
        result[k] = int(new_exp)
    return _clean(result)


def _pow_exponent_fraction(exponent) -> Fraction:
    """Convert a SymPy power exponent to an exact-enough Fraction."""
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
        The resulting Dimensions dict.

    Raises:
        KeyError: If a symbol is not found in dim_map.
        DimensionalError: If dimensions are inconsistent (e.g. in addition).
    """
    if isinstance(expr, Symbol):
        name = expr.name
        if name not in dim_map:
            raise KeyError(name)
        return _clean(dict(dim_map[name]))

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
        exponent = expr.args[1]

        if is_dimensionless(base_dims):
            return {}

        # Convert exponent to Fraction for exact arithmetic.
        exp_frac = _pow_exponent_fraction(exponent)

        if exp_frac.denominator == 1:
            # Integer power — use simple multiplication
            return _clean({k: v * int(exp_frac) for k, v in base_dims.items()})
        else:
            return _pow_dims_frac(base_dims, exp_frac)

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


def _kind_details_of_symbol(
    expr: Symbol,
    registry: KindRegistry,
    kind_map: dict[str, str],
) -> _KindDetails:
    name = expr.name
    if name not in kind_map:
        raise UnknownKindError(f"Missing kind binding for symbol: {name}")

    kind = kind_map[name]
    return _KindDetails(kind, registry.kind_dimensions(kind), (f"symbol {name} -> {kind}",))


def _combine_kind_details(
    left: _KindDetails,
    op: OperationName,
    right: _KindDetails,
    registry: KindRegistry,
) -> _KindDetails:
    if left.kind is None and right.kind is None:
        return _KindDetails(None, {}, left.steps + right.steps)
    if op == "mul" and left.kind is None:
        return right
    if op == "mul" and right.kind is None:
        return left
    if op == "div" and right.kind is None:
        return left
    if left.kind is None or right.kind is None:
        raise MissingOperationRuleError(f"No operation rule for scalar {op} {right.kind}")

    rule = registry.operation_rule(left.kind, op, right.kind)
    result_kind = rule.result_kind
    step = f"{left.kind} {op} {right.kind} -> {result_kind}"
    if rule.rationale:
        step = f"{step}; {rule.rationale}"
    return _KindDetails(
        result_kind,
        registry.kind_dimensions(result_kind),
        left.steps + right.steps + (step,),
    )


def _is_reciprocal(expr) -> bool:
    return isinstance(expr, Pow) and expr.args[1] == Integer(-1)


def _same_kind_args(
    expr,
    registry: KindRegistry,
    kind_map: dict[str, str],
    context: str,
) -> _KindDetails:
    details = [_kind_details_of_expr(arg, registry, kind_map) for arg in expr.args]
    if not details:
        return _KindDetails(None, {})

    first = details[0]
    for i, current in enumerate(details[1:], 1):
        if first.kind != current.kind or not dims_equal(first.dimensions, current.dimensions):
            raise KindMismatchError(
                f"Kind mismatch in {context}: argument 0 has {first.kind} "
                f"with {first.dimensions}, argument {i} has {current.kind} "
                f"with {current.dimensions}"
            )
    return first


def _dimensionless_function_kind_details(
    expr,
    registry: KindRegistry,
    kind_map: dict[str, str],
) -> _KindDetails:
    for arg in expr.args:
        arg_details = _kind_details_of_expr(arg, registry, kind_map)
        if not is_dimensionless(arg_details.dimensions):
            raise DimensionalError(
                f"{expr.func.__name__} argument must be dimensionless; "
                f"got {arg_details.dimensions}"
            )
    steps = tuple(step for detail in [_kind_details_of_expr(arg, registry, kind_map) for arg in expr.args] for step in detail.steps)
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
        return _same_kind_args(expr, registry, kind_map, "addition")

    if isinstance(expr, Mul):
        result = _KindDetails(None, {})
        for arg in expr.args:
            if _is_reciprocal(arg):
                right = _kind_details_of_expr(arg.args[0], registry, kind_map)
                result = _combine_kind_details(result, "div", right, registry)
            else:
                right = _kind_details_of_expr(arg, registry, kind_map)
                result = _combine_kind_details(result, "mul", right, registry)
        return result

    if isinstance(expr, Pow):
        base = _kind_details_of_expr(expr.args[0], registry, kind_map)
        exponent = expr.args[1]
        if base.kind is None:
            return _KindDetails(None, {}, base.steps)

        exp_frac = _pow_exponent_fraction(exponent)
        if exp_frac.denominator != 1:
            raise DimensionalError(
                f"fractional exponent in kind expression is not supported: {exponent}"
            )

        exp_int = int(exp_frac)
        if exp_int == 1:
            return base

        result_dims = pow_dims(base.dimensions, exp_int)
        try:
            result_kind = registry.unique_kind_with_dimensions(result_dims)
        except AmbiguousKindError:
            raise
        return _KindDetails(
            result_kind,
            registry.kind_dimensions(result_kind),
            base.steps + (f"{base.kind} pow {exp_int} -> {result_kind}",),
        )

    if isinstance(expr, Relational):
        raise KindMismatchError("Nested relational expressions are not kind terms")

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
        return _kind_details_of_expr(expr.args[0], registry, kind_map)

    if getattr(expr, "func", None) in {Min, Max}:
        return _same_kind_args(expr, registry, kind_map, expr.func.__name__)

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
    return lhs.kind == rhs.kind and dims_equal(lhs.dimensions, rhs.dimensions)


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
    except MissingOperationRuleError as exc:
        return CheckResult(
            False,
            lhs_kind=lhs.kind if lhs else None,
            lhs_dimensions=lhs.dimensions if lhs else None,
            reason=f"missing operation rule: {exc}",
            steps=(str(exc),) if lhs is None else lhs.steps + (str(exc),),
        )
    except KindMismatchError as exc:
        return CheckResult(
            False,
            lhs_kind=lhs.kind if lhs else None,
            lhs_dimensions=lhs.dimensions if lhs else None,
            reason=f"kind mismatch: {exc}",
            steps=(str(exc),) if lhs is None else lhs.steps + (str(exc),),
        )
    except Exception as exc:
        return CheckResult(
            False,
            lhs_kind=lhs.kind if lhs else None,
            lhs_dimensions=lhs.dimensions if lhs else None,
            reason=str(exc),
            steps=(str(exc),) if lhs is None else lhs.steps + (str(exc),),
        )

    steps = lhs.steps + rhs.steps
    if lhs.kind == rhs.kind and dims_equal(lhs.dimensions, rhs.dimensions):
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
