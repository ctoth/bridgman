"""Reject source archives missing declaration inputs or carrying bytecode."""

from pathlib import Path
import sys
import tarfile


with tarfile.open(Path(sys.argv[1])) as archive:
    names = {name.partition("/")[2] for name in archive.getnames()}
required = {
    "profiles/thermal.yml",
    "design/quantity-contract-cases.yml",
    "tests/test_symbolic.py",
    "src/bridgman/_core.pyi",
    "src/bridgman/py.typed",
    "tools/generate_profile.py",
}
assert required <= names, f"Missing source inputs: {required - names}"
assert not any("__pycache__" in name or name.endswith((".pyc", ".pyo")) for name in names)
print("Source archive includes declarations, contracts and typing without bytecode")
