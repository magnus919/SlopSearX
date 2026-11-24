"""Expanded scholarly records retain bounded attributed member evidence."""

from copy import deepcopy

from slopsearx.mcp.result_serialization import _work_group_for_record
from slopsearx.scholarly import MAX_MEMBERS, MAX_RECORD_BYTES


def test_group_is_bounded_and_copied():
    group = {"schema_version": 1, "members": [{"engine": "arxiv", "payload": {"value": "one"}}]}
    original = deepcopy(group)
    output = _work_group_for_record(group)
    assert output == group
    output["members"][0]["payload"]["value"] = "changed"
    assert group == original
    for invalid in [
        None,
        {},
        {"schema_version": 1, "members": []},
        {"schema_version": 1, "members": [{"engine": 1}]},
        {"schema_version": 1, "members": [{"engine": "a"}] * (MAX_MEMBERS + 1)},
        {"schema_version": 1, "members": [{"engine": "a", "data": "x" * MAX_RECORD_BYTES}]},
    ]:
        assert _work_group_for_record(invalid) is None
