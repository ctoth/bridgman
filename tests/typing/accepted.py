from bridgman import Dimensions, dims_signature, mul_dims, pi_groups

length: Dimensions = {"L": 1}
time: Dimensions = {"T": 1}
velocity: Dimensions = mul_dims(length, {"T": -1})
signature: str = dims_signature(velocity)
groups: tuple[dict[str, int], ...] = pi_groups({"length": length, "time": time})
