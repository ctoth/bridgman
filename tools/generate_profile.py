from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import tempfile
import yaml

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "profiles" / "thermal.yml"
DEST = ROOT / "crates" / "bridgman-core" / "src"


def render() -> dict[str, str]:
    profile = yaml.safe_load(SOURCE.read_text(encoding="utf-8"))
    kinds = profile["kinds"]
    dimensionless = profile["dimensionless_kind"]
    affine = profile["affine_spaces"]
    roots = profile["roots"]
    names = ",\n    ".join(kind["name"] for kind in kinds)
    dimensions = "\n".join(
        f"            Self::{kind['name']} => {kind['dimensions']}," for kind in kinds
    )
    linear = ",\n    ".join(kind["name"] for kind in kinds if kind["linear"])
    linear_matches = " | ".join(f"Kind::{kind['name']}" for kind in kinds if kind["linear"])
    kinds_rs = f"""// Generated from profiles/thermal.yml; do not edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = \"snake_case\")]
pub enum Kind {{
    {names}
}}
impl Kind {{
    pub const fn dimensions(self) -> [i8; 7] {{
        match self {{
{dimensions}
        }}
    }}
}}
kinds!(
    {names}
);
linear!(
    {linear}
);
fn kind_is_linear(kind: Kind) -> bool {{ matches!(kind, {linear_matches}) }}
"""
    units = "\n".join(
        f'    {name}:{kind}="{symbol}",{n},{d},{on},{od};'
        for name, kind, symbol, n, d, on, od in profile["units"]
    )
    units_rs = f"// Generated from profiles/thermal.yml; do not edit.\nunits! {{\n{units}\n}}\n"
    arms = "\n".join(
        f"        (Op::{op}, {left}, {right}) => Ok({result}),"
        for op, left, right, result in profile["rules"]
    )
    impls = "\n".join(
        f"operation!({op}, {op.lower()}, {'multiply' if op == 'Mul' else 'divide'}, {left}, {right}, {result});"
        for op, left, right, result in profile["rules"]
    )
    affine_arms = "\n".join(
        f"        (Op::Add, {space['point']}, {space['difference']}) | (Op::Add, {space['difference']}, {space['point']}) => Ok({space['point']}),\n"
        f"        (Op::Sub, {space['point']}, {space['point']}) => Ok({space['difference']}),\n"
        f"        (Op::Sub, {space['point']}, {space['difference']}) => Ok({space['point']}),"
        for space in affine
    )
    point_kinds = " | ".join(space["point"] for space in affine)
    affine_impls = "\n".join(
        f"operation!(Add, add, checked_add, {space['point']}, {space['difference']}, {space['point']});\n"
        f"operation!(Add, add, checked_add, {space['difference']}, {space['point']}, {space['point']});\n"
        f"operation!(Sub, sub, subtract, {space['point']}, {space['point']}, {space['difference']});\n"
        f"operation!(Sub, sub, subtract, {space['point']}, {space['difference']}, {space['point']});"
        for space in affine
    )
    bound_checks = "\n".join(
        f"    if kind == Kind::{bound['kind']} && value < {float(bound['lower']):.1f} {{ return Err(QuantityError::{bound['error']}); }}"
        for bound in profile["bounds"]
    )
    root_impls = "\n".join(
        f"impl Quantity<{root['kind']}> {{\n"
        f"    pub fn sqrt(self) -> Result<Quantity<{root['result']}>, QuantityError> {{\n"
        f"        if self.canonical < 0.0 {{ return Err(QuantityError::{root['negative_error']}); }}\n"
        f"        Quantity::computed(self.canonical.sqrt())\n"
        f"    }}\n"
        f"}}\n"
        f"fn sqrt_dynamic(q: AnyQuantity) -> Result<AnyQuantity, QuantityError> {{\n"
        f"    if q.kind != Kind::{root['kind']} {{ return Err(QuantityError::UnsupportedOperation {{ operation: Operation::Sqrt, left: q.kind, right: None }}); }}\n"
        f"    q.try_typed::<{root['kind']}>()?.sqrt().map(Into::into)\n"
        f"}}"
        for root in roots
    )
    operations_rs = f"""// Generated from profiles/thermal.yml; do not edit.
pub fn binary_kind(a: Kind, b: Kind, op: Op) -> Result<Kind, QuantityError> {{
    use Kind::*;
    match (op, a, b) {{
{affine_arms}
        (Op::Add | Op::Sub, x, y) if x == y && !matches!(x, {point_kinds}) => Ok(x),
{arms}
        (Op::Mul | Op::Div, x, {dimensionless}) if !matches!(x, {point_kinds}) => Ok(x),
        (Op::Mul, {dimensionless}, x) if !matches!(x, {point_kinds}) => Ok(x),
        (Op::Div, x, y) if x == y && !matches!(x, {point_kinds}) => Ok({dimensionless}),
        _ => Err(QuantityError::UnsupportedOperation {{ operation: Operation::Binary(op), left: a, right: Some(b) }}),
    }}
}}
fn checked(kind: Kind, value: f64) -> Result<(), QuantityError> {{
    if !value.is_finite() {{ return Err(QuantityError::NumericalFailure); }}
{bound_checks}
    Ok(())
}}
{root_impls}
{affine_impls}
{impls}
impl<K: Linear> Div for Quantity<K> {{
    type Output = Result<Quantity<{dimensionless}>, QuantityError>;
    fn div(self, b: Self) -> Self::Output {{
        AnyQuantity::from(self).divide(b.into())?.try_typed()
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
