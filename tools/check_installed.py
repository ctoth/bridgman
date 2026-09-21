"""Check the installed native package and its positive/negative typing contract."""

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import bridgman
from bridgman import _core


package = Path(bridgman.__file__).resolve().parent
assert (package / "py.typed").is_file()
assert (package / "_core.pyi").is_file()
assert Path(_core.__file__).parent == package
assert _core.mul_dims({"L": 10**40}, {"L": 1}) == {"L": 10**40 + 1}

fixtures = Path(__file__).resolve().parents[1] / "tests" / "typing"
with tempfile.TemporaryDirectory(prefix="bridgman-installed-typing-") as directory:
    temporary = Path(directory)
    for name, expected in (("accepted", 0), ("rejected", 1)):
        path = temporary / f"{name}.py"
        shutil.copyfile(fixtures / path.name, path)
        result = subprocess.run(
            [sys.executable, "-m", "pyright", "--pythonpath", sys.executable, "--outputjson", str(path)],
            cwd=temporary,
            capture_output=True,
            text=True,
        )
        if result.returncode != expected:
            raise AssertionError(result.stdout + result.stderr)
        report = json.loads(result.stdout)
        if (report["summary"]["errorCount"] > 0) != bool(expected):
            raise AssertionError(report)
print(f"Installed native and typing contracts passed: {package}")
