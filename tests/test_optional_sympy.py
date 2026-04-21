"""Optional sympy dependency behavior."""

from __future__ import annotations

import builtins
import importlib
import sys

import pytest


def test_bridgman_imports_without_sympy(monkeypatch):
    """Package root import still exposes symbolic stubs when sympy is absent."""
    original_import = builtins.__import__
    previous_modules = {
        name: sys.modules[name]
        for name in ("bridgman", "bridgman.symbolic")
        if name in sys.modules
    }

    def import_without_sympy(name, globals=None, locals=None, fromlist=(), level=0):
        if name == "sympy" or name.startswith("sympy."):
            raise ImportError("No module named 'sympy'")
        return original_import(name, globals, locals, fromlist, level)

    monkeypatch.setattr(builtins, "__import__", import_without_sympy)
    sys.modules.pop("bridgman", None)
    sys.modules.pop("bridgman.symbolic", None)

    try:
        bridgman = importlib.import_module("bridgman")

        assert "verify_expr" in bridgman.__all__
        assert "SympyRequiredError" in bridgman.__all__
        with pytest.raises(bridgman.SympyRequiredError, match="install.*sympy"):
            bridgman.verify_expr("F=m*a")
    finally:
        sys.modules.pop("bridgman", None)
        sys.modules.pop("bridgman.symbolic", None)
        sys.modules.update(previous_modules)
