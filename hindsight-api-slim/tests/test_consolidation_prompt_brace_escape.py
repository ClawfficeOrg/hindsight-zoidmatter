"""
Tests for brace escaping in the consolidation prompt builder.

The assembled prompt is later passed through ``str.format`` to substitute
real placeholders (``{facts_text}`` / ``{observations_text}``). Caller-
supplied text — ``observations_mission`` and ``observation_capacity_note`` —
must not crash the formatter when it happens to contain literal braces (e.g.
a JSON example).
"""

import pytest

from hindsight_api.engine.consolidation.prompts import (
    _escape_braces,
    build_batch_consolidation_prompt,
)


def _render(prompt: str) -> str:
    """Render the assembled prompt the way the consolidator does."""
    return prompt.format(facts_text="<facts>", observations_text="<observations>")


class TestEscapeBraces:
    def test_lone_open_brace_doubled(self):
        assert _escape_braces("{x}") == "{{x}}"

    def test_already_escaped_left_alone(self):
        assert _escape_braces("{{x}}") == "{{x}}"

    def test_idempotent_under_repeat(self):
        once = _escape_braces('{"dedup": true}')
        twice = _escape_braces(once)
        assert once == twice

    def test_no_braces_unchanged(self):
        assert _escape_braces("just prose, no braces") == "just prose, no braces"

    def test_mixed_lone_and_escaped(self):
        # Lone {x} should be escaped; existing {{y}} should stay.
        assert _escape_braces("{x} and {{y}}") == "{{x}} and {{y}}"


class TestBuildBatchConsolidationPromptBraceSafety:
    def test_default_mission_renders(self):
        prompt = build_batch_consolidation_prompt()
        rendered = _render(prompt)
        assert "<facts>" in rendered
        assert "<observations>" in rendered

    def test_mission_with_json_example_does_not_crash(self):
        """Reproduces the failure mode where a mission containing literal
        JSON braces was interpreted as a format placeholder."""
        mission = '{"dedup": true, "merge": true, "trend_tracking": false}'
        prompt = build_batch_consolidation_prompt(observations_mission=mission)
        rendered = _render(prompt)
        # The original mission text appears verbatim in the rendered prompt.
        assert mission in rendered

    def test_mission_with_multiple_brace_pairs(self):
        mission = "Example schema: {a: 1} and counter-example: {b: 2}"
        prompt = build_batch_consolidation_prompt(observations_mission=mission)
        rendered = _render(prompt)
        assert "{a: 1}" in rendered
        assert "{b: 2}" in rendered

    def test_capacity_note_with_braces(self):
        # observation_capacity_note is server-generated today, but the same
        # escape contract applies in case future call sites widen the input.
        note = "Use shape {limit, used}"
        prompt = build_batch_consolidation_prompt(
            observations_mission="m",
            observation_capacity_note=note,
        )
        rendered = _render(prompt)
        assert "{limit, used}" in rendered

    def test_already_escaped_mission_renders_to_literal_braces(self):
        """If a caller pre-escaped the mission (e.g. as a temporary data fix
        applied before this code rolled out), the rendered prompt must still
        contain the original single braces — not a double-escape artefact."""
        original = '{"dedup": true}'
        pre_escaped = '{{"dedup": true}}'
        prompt = build_batch_consolidation_prompt(observations_mission=pre_escaped)
        rendered = _render(prompt)
        assert original in rendered
        assert "{{" not in rendered.split("## MISSION")[1].split("##")[0]

    def test_mission_without_braces_unchanged(self):
        mission = "Track project deadlines and named contributors."
        prompt = build_batch_consolidation_prompt(observations_mission=mission)
        rendered = _render(prompt)
        assert mission in rendered

    def test_unaffected_format_placeholders_still_substitute(self):
        """The fix must not break the existing {facts_text} / {observations_text}
        substitution path."""
        prompt = build_batch_consolidation_prompt(observations_mission="Note: {x: 1}")
        rendered = prompt.format(facts_text="FACTS_HERE", observations_text="OBS_HERE")
        assert "FACTS_HERE" in rendered
        assert "OBS_HERE" in rendered

    @pytest.mark.parametrize(
        "mission",
        [
            "{single}",
            "}}weird{{",
            "trailing {",
            "leading }",
            "",
        ],
    )
    def test_assorted_mission_inputs_do_not_crash(self, mission):
        prompt = build_batch_consolidation_prompt(observations_mission=mission)
        # Just ensure format does not raise.
        prompt.format(facts_text="f", observations_text="o")
