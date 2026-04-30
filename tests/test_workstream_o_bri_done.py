from __future__ import annotations

import pytest


@pytest.mark.xfail(reason="WS-O-bri closure sentinel flips in final commit")
def test_workstream_o_bri_done() -> None:
    assert False
