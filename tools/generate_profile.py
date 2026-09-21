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
    names = ",\n    ".join(kind["name"] for kind in kinds)
    dimensions = "\n".join(
        f"            Self::{kind['name']} => {kind['dimensions']}," for kind in kinds
    )
    linear = ",\n    ".join(kind["name"] for kind in kinds if kind["linear"])
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
    affine_impls = """operation!(Add, add, checked_add, Temperature, TemperatureDelta, Temperature);
operation!(Add, add, checked_add, TemperatureDelta, Temperature, Temperature);
operation!(Sub, sub, subtract, Temperature, Temperature, TemperatureDelta);
operation!(Sub, sub, subtract, Temperature, TemperatureDelta, Temperature);"""
    operations_rs = f"""// Generated from profiles/thermal.yml; do not edit.
pub fn binary_kind(a: Kind, b: Kind, op: Op) -> Result<Kind, QuantityError> {{
    use Kind::*;
    match (op, a, b) {{
        (Op::Add, Temperature, TemperatureDelta) | (Op::Add, TemperatureDelta, Temperature) => Ok(Temperature),
        (Op::Sub, Temperature, Temperature) => Ok(TemperatureDelta),
        (Op::Sub, Temperature, TemperatureDelta) => Ok(Temperature),
        (Op::Add | Op::Sub, x, y) if x == y && x != Temperature => Ok(x),
{arms}
        (Op::Mul | Op::Div, x, Unitless) if x != Temperature => Ok(x),
        (Op::Mul, Unitless, x) if x != Temperature => Ok(x),
        (Op::Div, x, y) if x == y && x != Temperature => Ok(Unitless),
        _ => Err(QuantityError::UnsupportedOperation {{ operation: Operation::Binary(op), left: a, right: Some(b) }}),
    }}
}}
{affine_impls}
{impls}
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
