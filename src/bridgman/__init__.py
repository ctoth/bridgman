"""Bridgman: dimensional analysis arithmetic for SI quantities."""

from typing import TYPE_CHECKING

from bridgman._core import (
    BridgmanError,
    CatalogError,
    DerivationError,
    DimensionError,
    OperationParseError,
    QuantityError,
)
from bridgman.dimensions import (
    SI_BASES,
    Dimensions,
    common_dims,
    transcendental_dims,
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
    CheckResult,
    KindRegistry,
    OperationRule,
    QuantityKind,
)
from bridgman.pi import PiError, count_pi_groups, is_dimensionless_product, pi_groups

__version__ = "0.2.0"


class SympyRequiredError(ImportError):
    """Raised when symbolic APIs are used without the optional sympy dependency."""


__all__ = [
    "SI_BASES",
    "Dimensions",
    "common_dims",
    "transcendental_dims",
    "mul_dims",
    "div_dims",
    "pow_dims",
    "dims_equal",
    "is_dimensionless",
    "format_dims",
    "dims_signature",
    "parse_dims_signature",
    "canonicalize_dims",
    "BridgmanError",
    "CatalogError",
    "DerivationError",
    "DimensionError",
    "OperationParseError",
    "QuantityError",
    "CheckResult",
    "KindRegistry",
    "OperationRule",
    "QuantityKind",
    "PiError",
    "count_pi_groups",
    "is_dimensionless_product",
    "pi_groups",
    "SympyRequiredError",
    "__version__",
]

if TYPE_CHECKING:
    from bridgman.symbolic import (
        UnsupportedExpressionError,
        dims_of_expr,
        explain_expr,
        explain_expr_kinds,
        kind_of_expr,
        verify_expr,
        verify_expr_kinds,
    )
else:
    try:
        from bridgman.symbolic import (
            UnsupportedExpressionError,
            dims_of_expr,
            explain_expr,
            explain_expr_kinds,
            kind_of_expr,
            verify_expr,
            verify_expr_kinds,
        )

    except ImportError as exc:
        if exc.name != "sympy" and not (exc.name and exc.name.startswith("sympy.")):
            raise

        class UnsupportedExpressionError(TypeError):
            """Raised for a sympy construct the walker cannot read."""

        def dims_of_expr(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

        def verify_expr(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

        def kind_of_expr(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

        def verify_expr_kinds(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

        def explain_expr(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

        def explain_expr_kinds(*_args, **_kwargs):
            raise SympyRequiredError("install bridgman[sympy] to use symbolic expressions")

__all__ += [
    "dims_of_expr",
    "verify_expr",
    "kind_of_expr",
    "verify_expr_kinds",
    "explain_expr",
    "explain_expr_kinds",
    "UnsupportedExpressionError",
]
