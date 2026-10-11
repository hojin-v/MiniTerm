---
name: miniterm-progress
description: "Report the current task and explicit plan progress to MiniTerm while doing multi-step work inside a MiniTerm terminal. Use when MiniTerm progress reporting is requested or MINITERM_SESSION_ID and MINITERM_NOTIFY_EXE are available."
---

# MiniTerm Progress

Keep MiniTerm's compact summary aligned with the work you are actually doing. Continue the user's task normally; this skill adds concise progress reports, not a different planning workflow.

Use `scripts/report.py` via Python, supplying a UTF-8 JSON report on stdin. The helper checks the inherited MiniTerm session and `progress-v1` capability before reporting. If Python, the helper, or that capability is unavailable, say once that structured progress is unavailable and continue the task. Do not launch another MiniTerm, change global agent configuration, or repeatedly retry an unavailable integration.

For a new user request, send `begin` with the current explicit plan. Keep the returned `workId` and `revision`. Send `update --work-id <id> --revision <previous+1>` when the current task changes, an item finishes, the plan changes, or work pauses/ends. Send the whole current plan each time; do not report every token, shell command, or heartbeat. A resumed task can include previously verified completed items.

Report schema:

```json
{
  "title": "한글 입력 문제 수정",
  "state": "running",
  "tasks": [
    {"text": "재현 조건 확인", "state": "completed"},
    {"text": "입력 처리 수정", "state": "running"},
    {"text": "실제 터미널 검증", "state": "pending"}
  ]
}
```

`state` is `running`, `completed`, `paused`, or `failed`. Task states are `pending`, `running`, `completed`, or `stopped`. Use short task labels (up to 256 characters); omit full prompts, source code, terminal transcripts, credentials, and personal details. The title is a short work label (up to 128 characters), not a guessed Codex session name. Reports support 1–64 items within 48 KiB.

Mark an item `completed` only after its actual work and required verification finish. Report `completed` for the plan only when every item is completed. Stopping a response, handing work back, or exhausting context does not finish pending tasks. Use `paused` for a pause or a genuine wait, and `failed` for a work failure; retain incomplete items. Do not manufacture percentages: MiniTerm calculates the completed-item ratio. This ratio is not a time estimate.

Example in Bash (replace the script path with this skill's actual path):

```bash
python /path/to/miniterm-progress/scripts/report.py begin <<'JSON'
{"title":"입력 문제 수정","state":"running","tasks":[{"text":"재현","state":"completed"},{"text":"수정과 검증","state":"running"}]}
JSON
```

In Windows PowerShell 5, set the pipeline encoding for this call so Korean is not replaced with `?`:

```powershell
$taskPreviousEncoding = $OutputEncoding
try {
    $OutputEncoding = [Text.UTF8Encoding]::new($false)
    '{"title":"입력 문제 수정","state":"running","tasks":[{"text":"수정과 검증","state":"running"}]}' | python C:/path/to/miniterm-progress/scripts/report.py begin
} finally { $OutputEncoding = $taskPreviousEncoding }
```

Bash UTF-8 heredocs also work without escaping Korean. Substitute the real skill path and actual authorized plan.

MiniTerm expires reports when the user submits input with Enter or Ctrl+C. A receipt from before that input cannot update the summary. If a report fails because its receipt expired, start a fresh `begin` for the work you are currently authorized to continue. Do not revive the previous request after the user switches tasks. For an interrupted turn with no opportunity to report, MiniTerm may retain only a last reported plan until user input expires it; never describe skill reporting as an automatic guarantee.

`read` explicitly returns this terminal's report; `clear --work-id <id> --revision <previous+1>` removes it. Neither is needed for routine updates. The helper never discovers other users' terminal content or reads agent transcript files.
