"""Bridgman: dimensional analysis arithmetic for SI quantities."""

from typing import TYPE_CHECKING

from bridgman.dimensions import (
    Dimensions,
    mul_dims,
    div_dims,
    pow_dims,
    dims_equal,
    is_dimensionless,
    format_dims,
    dims_signature,
    parse_dims_signature,
    canonicalize_dims,
)
from bridgman.kinds import (
    AmbiguousKindError,
    DuplicateKindError,
    DuplicateOperationRuleError,
    InvalidOperationRuleError,
    KindError,
    KindMismatchError,
    KindRegistry,
    MissingOperationRuleError,
    OperationRule,
    QuantityKind,
    UnknownKindError,
)

__version__ = "0.2.0"


class SympyRequiredError(ImportError):
    """Raised when symbolic APIs are used without the optional sympy dependency."""


__all__ = [
    "Dimensions",
    "mul_dims",
    "div_dims",
    "pow_dims",
    "dims_equal",
    "is_dimensionless",
    "format_dims",
    "dims_signature",
    "parse_dims_signature",
    "canonicalize_dims",
    "AmbiguousKindError",
    "DuplicateKindError",
    "DuplicateOperationRuleError",
    "InvalidOperationRuleError",
    "KindError",
    "KindMismatchError",
    "KindRegistry",
    "MissingOperationRuleError",
    "OperationRule",
    "QuantityKind",
    "UnknownKindError",
    "SympyRequiredError",
    "__version__",
]

if TYPE_CHECKING:
    from bridgman.symbolic import (
        DimensionalError,
        dims_of_expr,
        kind_of_expr,
        verify_expr,
        verify_expr_kinds,
    )
else:
    try:
        from bridgman.symbolic import (
            DimensionalError,
            dims_of_expr,
            kind_of_expr,
            verify_expr,
            verify_expr_kinds,
        )

    except ImportError as exc:
        if exc.name != "sympy" and not (exc.name and exc.name.startswith("sympy.")):
            raise

        class DimensionalError(Exception):
            """Raised when dimensions are inconsistent in symbolic expressions."""

        def dims_of_expr(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

        def verify_expr(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

        def kind_of_expr(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

        def verify_expr_kinds(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

__all__ += ["dims_of_expr", "verify_expr", "kind_of_expr", "verify_expr_kinds", "DimensionalError"]
