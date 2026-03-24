# bridgman

Dimensional analysis arithmetic. Named after [Percy Bridgman](https://en.wikipedia.org/wiki/Percy_Williams_Bridgman).

Does one thing: SI dimension exponent math. Multiply quantities, divide them, check if equations are dimensionally consistent.

## Install

`pip install bridgman` (or add as path dependency)

## Usage

```python
from bridgman import mul_dims, div_dims, verify_equation

force = {"M": 1, "L": 1, "T": -2}
mass = {"M": 1}
accel = {"L": 1, "T": -2}

assert mul_dims(mass, accel) == force
assert verify_equation(force, [mass, accel], ["mul"])
```

## Why

Because `F = m * a` should be verifiable at the dimensional level without importing a physics engine.

## License

MIT
