from fractions import Fraction

from bridgman import Dimensions, dims_signature, mul_dims, pi_groups, pow_dims
from bridgman import BridgmanError, KindRegistry, QuantityError

length: Dimensions = {"L": 1}
time: Dimensions = {"T": 1}
velocity: Dimensions = mul_dims(length, {"T": -1})
root: Dimensions = pow_dims(length, Fraction(1, 2))
signature: str = dims_signature(velocity)
groups: tuple[dict[str, int], ...] = pi_groups({"length": length, "time": time})
kind: str = KindRegistry.bundled().result_kind("force", "dot", "displacement")
refused: type[BridgmanError] = QuantityError.KindMismatch
variants: tuple[str, ...] = QuantityError.variants
