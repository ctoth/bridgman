from bridgman import KindRegistry, pow_dims

pow_dims({"L": 1}, "two")
pow_dims({"L": 1}, 0.5)
KindRegistry.bundled().rule_rationale("force", "add", "displacement")
