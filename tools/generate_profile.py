from __future__ import annotations

import argparse
import json
import math
import re
from pathlib import Path
import subprocess
import tempfile
import yaml

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "profiles" / "thermal.yml"
DEST = ROOT / "crates" / "bridgman-core" / "src"


def product(left: dict, right: dict, sign: int) -> dict:
    result = dict(left)
    for base, power in right.items():
        result[base] = result.get(base, 0) + sign * power
    return {base: power for base, power in result.items() if power != 0}


def points_of(profile: dict) -> set[str]:
    """Point kinds are those that declare an affine space; every other kind is linear."""
    return {space["point"] for space in profile["affine_spaces"]}


def validate(profile: dict) -> None:
    if profile["schema"] != 1:
        raise ValueError("unsupported profile schema")
    kinds = {kind["name"]: kind for kind in profile["kinds"]}
    if len(kinds) != len(profile["kinds"]):
        raise ValueError("duplicate kind")
    for name, kind in kinds.items():
        if not re.fullmatch(r"[A-Z][A-Za-z0-9_]*", name):
            raise ValueError("invalid Rust kind identifier")
        dimensions = kind["dimensions"]
        if not isinstance(dimensions, dict) or any(
            not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", base) or type(power) is not int or power == 0
            for base, power in dimensions.items()
        ):
            raise ValueError("dimensions map base identifiers to nonzero integer powers")
    points = points_of(profile)
    differences = {space["difference"] for space in profile["affine_spaces"]}
    if len(points) != len(profile["affine_spaces"]) or points & differences:
        raise ValueError("invalid affine space")
    for space in profile["affine_spaces"]:
        if kinds[space["point"]]["dimensions"] != kinds[space["difference"]]["dimensions"]:
            raise ValueError("invalid affine space")
    identity = profile["dimensionless_kind"]
    if kinds[identity]["dimensions"] or identity in points:
        raise ValueError("identity must be linear and dimensionless")
    seen = set()
    for op, left, right, result in profile["rules"]:
        key = op, left, right
        if op not in {"Mul", "Div"} or key in seen or any(name in points for name in (left, right, result)):
            raise ValueError("invalid or duplicate product rule")
        seen.add(key)
        expected = product(kinds[left]["dimensions"], kinds[right]["dimensions"], 1 if op == "Mul" else -1)
        if expected != kinds[result]["dimensions"]:
            raise ValueError("dimensionally invalid product rule")
    root_kinds = set()
    for root in profile["roots"]:
        if root["degree"] != 2 or root["kind"] in root_kinds:
            raise ValueError("only one declared square root per kind is supported")
        root_kinds.add(root["kind"])
        result = kinds[root["result"]]["dimensions"]
        if kinds[root["kind"]]["dimensions"] != product(result, result, 1):
            raise ValueError("dimensionally invalid square root")
    for bound in profile["bounds"]:
        if bound["kind"] not in kinds or not math.isfinite(float(bound["lower"])):
            raise ValueError("invalid bound")
    names, symbols = set(), set()
    for name, kind, symbol, n, d, on, od in profile["units"]:
        if name in names or symbol in symbols or any(ord(c) < 32 for c in symbol) or kind not in kinds or not re.fullmatch(r"[A-Z][A-Z0-9_]*", name):
            raise ValueError("invalid or ambiguous unit")
        if any(type(v) is not int or not -(2**63) <= v < 2**63 for v in (n, d, on, od)) or n == 0 or d == 0 or od == 0:
            raise ValueError("invalid rational unit transform")
        if kind not in points and on != 0:
            raise ValueError("linear units cannot have offsets")
        names.add(name)
        symbols.add(symbol)


def render(profile: dict | None = None) -> dict[str, str]:
    if profile is None:
        profile = yaml.safe_load(SOURCE.read_text(encoding="utf-8"))
    validate(profile)
    kinds = profile["kinds"]
    points = points_of(profile)
    dimensionless = profile["dimensionless_kind"]
    affine = profile["affine_spaces"]
    roots = profile["roots"]
    declarations = "\n".join(
        "    {name} {{{powers}}},".format(
            name=kind["name"],
            powers="".join(f" {json.dumps(base)}: {power}," for base, power in kind["dimensions"].items()).rstrip(",") + " "
            if kind["dimensions"]
            else "",
        )
        for kind in kinds
    )
    linear = ", ".join(kind["name"] for kind in kinds if kind["name"] not in points)
    kinds_rs = f"""// Generated from profiles/thermal.yml; do not edit.
kinds! {{
{declarations}
}}
linear!({linear});
"""
    units = "\n".join(
        f'    {name}:{kind}={json.dumps(symbol, ensure_ascii=False)},{n},{d},{on},{od};'
        for name, kind, symbol, n, d, on, od in profile["units"]
    )
    units_rs = f"// Generated from profiles/thermal.yml; do not edit.\nunits! {{\n{units}\n}}\n"
    arms = "\n".join(
        f"        (Op::{op}, {left}, {right}) => Ok({result}),"
        for op, left, right, result in profile["rules"]
    )
    impls = "\n".join(
        f"operation!({op}, {op.lower()}, {left}, {right}, {result});"
        for op, left, right, result in profile["rules"]
    )
    affine_arms = "\n".join(
        f"        (Op::Add, {space['point']}, {space['difference']}) | (Op::Add, {space['difference']}, {space['point']}) => Ok({space['point']}),\n"
        f"        (Op::Sub, {space['point']}, {space['point']}) => Ok({space['difference']}),\n"
        f"        (Op::Sub, {space['point']}, {space['difference']}) => Ok({space['point']}),"
        for space in affine
    )
    affine_impls = "\n".join(
        f"operation!(Add, add, {space['point']}, {space['difference']}, {space['point']});\n"
        f"operation!(Add, add, {space['difference']}, {space['point']}, {space['point']});\n"
        f"operation!(Sub, sub, {space['point']}, {space['point']}, {space['difference']});\n"
        f"operation!(Sub, sub, {space['point']}, {space['difference']}, {space['point']});"
        for space in affine
    )
    bound_checks = "\n".join(
        f"    if kind == Kind::{bound['kind']} && value < {float(bound['lower'])!r} {{ return Err(QuantityError::{bound['error']}); }}"
        for bound in profile["bounds"]
    )
    root_impls = "\n".join(
        f"impl Quantity<{root['kind']}> {{\n"
        f"    pub fn sqrt(self) -> Result<Quantity<{root['result']}>, QuantityError> {{\n"
        f"        if self.canonical < 0.0 {{ return Err(QuantityError::{root['negative_error']}); }}\n"
        f"        Quantity::computed(self.canonical.sqrt())\n"
        f"    }}\n"
        f"}}"
        for root in roots
    )
    root_arms = "\n".join(
        f"        Kind::{root['kind']} => q.try_typed::<{root['kind']}>()?.sqrt().map(Into::into),"
        for root in roots
    )
    operations_rs = f"""// Generated from profiles/thermal.yml; do not edit.
pub fn binary_kind(a: Kind, b: Kind, op: Op) -> Result<Kind, QuantityError> {{
    use Kind::*;
    match (op, a, b) {{
{affine_arms}
        (Op::Add | Op::Sub, x, y) if x == y && kind_is_linear(x) => Ok(x),
{arms}
        (Op::Mul | Op::Div, x, {dimensionless}) if kind_is_linear(x) => Ok(x),
        (Op::Mul, {dimensionless}, x) if kind_is_linear(x) => Ok(x),
        (Op::Div, x, y) if x == y && kind_is_linear(x) => Ok({dimensionless}),
        _ => Err(QuantityError::UnsupportedOperation {{ operation: Operation::Binary(op), left: a, right: Some(b) }}),
    }}
}}
fn checked(kind: Kind, value: f64) -> Result<(), QuantityError> {{
    if !value.is_finite() {{ return Err(QuantityError::NumericalFailure); }}
{bound_checks}
    Ok(())
}}
{root_impls}
fn sqrt_dynamic(q: AnyQuantity) -> Result<AnyQuantity, QuantityError> {{
    match q.kind {{
{root_arms}
        _ => Err(QuantityError::UnsupportedOperation {{ operation: Operation::Sqrt, left: q.kind, right: None }}),
    }}
}}
{affine_impls}
{impls}
impl<K: Linear> Div for Quantity<K> {{
    type Output = Result<Quantity<{dimensionless}>, QuantityError>;
    fn div(self, b: Self) -> Self::Output {{
        Quantity::computed(arithmetic(Op::Div, self.canonical, b.canonical)?)
    }}
}}
"""
    return {
        "profile_kinds.rs": rustfmt(kinds_rs),
        "profile_units.rs": rustfmt(units_rs),
        "profile_operations.rs": rustfmt(operations_rs),
    }


def rustfmt(content: str) -> str:
    with tempfile.NamedTemporaryFile("w", suffix=".rs", encoding="utf-8", delete=False) as file:
        file.write(content)
        path = Path(file.name)
    try:
        subprocess.run(["rustfmt", "--edition", "2021", str(path)], check=True)
        return path.read_text(encoding="utf-8")
    finally:
        path.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    stale = []
    for name, content in render().items():
        path = DEST / name
        if args.check:
            if not path.exists() or path.read_text(encoding="utf-8") != content:
                stale.append(name)
        else:
            path.write_text(content, encoding="utf-8", newline="\n")
    if stale:
        raise SystemExit("stale generated profile artifacts: " + ", ".join(stale))


if __name__ == "__main__":
    main()
