---
name: json-report
description: Summarize a JSON array of numbers using an approved Python sandbox script.
---

# JSON Report

Use when the user asks for a numeric summary. First load this Skill with `skill_read`.
Request `python_run` with `skill_id: json-report`, `path: scripts/report.py` and one
argument containing a JSON array, for example `[1,2,3]`.
The project service dispatches execution to the configured sandbox worker; do not
run a native shell command. Report the returned count, sum and mean. If execution
is denied or fails, describe that accurately instead of inventing a result.
