//! Compact views read selected VT rows without resizing the running program.
use alacritty_terminal::{
    event::EventListener,
    grid::Dimensions,
    index::{Column, Line},
    term::{cell::Flags, Term},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub text: String,
    pub state: TaskState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub tasks: Vec<Task>,
    pub complete: bool,
}
impl Plan {
    pub fn completed(&self) -> usize {
        self.tasks
            .iter()
            .filter(|task| task.state == TaskState::Completed)
            .count()
    }
    pub fn percent(&self) -> Option<u8> {
        (self.complete && !self.tasks.is_empty())
            .then(|| (self.completed() * 100 / self.tasks.len()) as u8)
    }
    pub fn current(&self) -> Option<&Task> {
        self.tasks
            .iter()
            .find(|task| task.state == TaskState::Running)
            .or_else(|| {
                self.tasks
                    .iter()
                    .find(|task| task.state == TaskState::Pending)
            })
    }
    pub fn last_completed(&self) -> Option<&Task> {
        self.tasks
            .iter()
            .rev()
            .find(|task| task.state == TaskState::Completed)
    }
    pub fn description(&self) -> String {
        let mut text = String::from("터미널에 표시된 최근 계획 · 완료 항목 수 기준\n");
        for task in &self.tasks {
            let state = match task.state {
                TaskState::Pending => "대기",
                TaskState::Running => "진행",
                TaskState::Completed => "완료",
                TaskState::Stopped => "중단",
            };
            text.push_str(&format!("{state} · {}\n", task.text));
        }
        if !self.complete {
            text.push_str("목록 일부가 생략되어 완료율을 표시하지 않습니다.");
        }
        text
    }
}

#[derive(Debug, Clone, Default)]
pub struct Summary {
    pub activity: Option<String>,
    pub operation: Option<String>,
    pub recent: Option<String>,
    pub plan: Option<Plan>,
    pub battery_row: Option<i32>,
    pub battery_text: Option<String>,
    pub battery_columns: Option<usize>,
    pub reported: Option<crate::progress::Record>,
}

impl Summary {
    pub fn with_report(mut self, record: Option<crate::progress::Record>) -> Self {
        if let Some(record) = record {
            self.plan = Some(Plan {
                tasks: record.report.tasks.clone(),
                complete: true,
            });
            self.reported = Some(record);
        }
        self
    }
}

struct TextRow {
    line: i32,
    text: String,
    wrapped: bool,
    columns_used: usize,
}

/// Read a bounded part of the existing VT grid; no quota requests, transcript
/// files, process polling, terminal resize or synthetic completion events.
pub fn summarize<T: EventListener>(term: &Term<T>) -> Summary {
    let start = -(term.grid().history_size().min(128) as i32);
    let rows: Vec<TextRow> = (start..term.screen_lines() as i32)
        .map(|line| {
            let mut text = String::new();
            let mut columns_used = 0;
            for col in 0..term.columns().min(512) {
                let cell = &term.grid()[Line(line)][Column(col)];
                if cell.flags.intersects(
                    Flags::HIDDEN | Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER,
                ) {
                    continue;
                }
                text.push(cell.c);
                if cell.c != ' ' {
                    columns_used = col + 1 + usize::from(cell.flags.contains(Flags::WIDE_CHAR));
                }
                if let Some(extra) = cell.zerowidth() {
                    text.extend(extra);
                }
            }
            let wrapped = term.grid()[Line(line)][Column(term.columns() - 1)]
                .flags
                .contains(Flags::WRAPLINE);
            TextRow {
                line,
                text: if wrapped {
                    text
                } else {
                    text.trim_end().to_owned()
                },
                wrapped,
                columns_used,
            }
        })
        .collect();
    // The editable prompt at the cursor is not a submitted work item.
    let prompt = rows.iter().rposition(|row| {
        row.line < term.grid().cursor.point.line.0
            && row.text.trim().starts_with("› ")
            && !chrome(row.text.trim())
    });
    let mut summary = Summary::default();
    for (index, row) in rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.line >= 0)
        .rev()
    {
        let text = row.text.trim();
        if summary.battery_row.is_none() && battery(text) {
            summary.battery_row = Some(row.line);
            summary.battery_text = Some(text.to_owned());
            summary.battery_columns = Some(row.columns_used);
        }
        if summary.activity.is_none()
            && prompt.is_none_or(|prompt| index > prompt)
            && activity(text)
        {
            let text = activity_text(text);
            summary.activity = Some(
                if let Some((text, _)) = text.split_once(" • esc to interrupt") {
                    format!("{text})")
                } else {
                    text.to_owned()
                },
            );
        }
    }
    // A later submitted Codex prompt starts a new work item. Do not attach the
    // previous turn's completed checklist to that new request.
    let mut fenced = false;
    let mut header = None;
    for (index, row) in rows.iter().enumerate() {
        let text = row.text.trim();
        if text.starts_with("```") {
            fenced = !fenced;
        } else if !fenced && plan_header(text) {
            header = Some(index);
        }
    }
    if let Some(index) = header {
        if prompt.is_none_or(|prompt| index > prompt) {
            summary.plan = parse_plan(&rows[index + 1..]);
            if term.columns() > 512 {
                if let Some(plan) = &mut summary.plan {
                    plan.complete = false;
                }
            }
        }
    }
    summary.operation = rows.iter().enumerate().rev().find_map(|(index, row)| {
        if prompt.is_some_and(|prompt| index <= prompt) {
            return None;
        }
        let text = row.text.trim();
        text.strip_prefix("• Ran ")
            .map(|command| format!("최근 활동: {}", command_activity(command)))
            .or_else(|| {
                text.strip_prefix("• Explored")
                    .map(|_| "최근 활동: 파일·코드 확인".to_owned())
            })
            .or_else(|| {
                text.strip_prefix("• Edited ")
                    .map(|_| "최근 활동: 파일 수정".to_owned())
            })
    });
    summary.recent = rows.iter().enumerate().rev().find_map(|(index, row)| {
        let text = row.text.trim();
        (!text.is_empty()
            && !chrome(text)
            && !battery(text)
            && !activity(text)
            && !plan_header(text)
            && task(text).is_none()
            && row.line < term.grid().cursor.point.line.0
            && !text.starts_with("› ")
            && prompt.is_none_or(|prompt| index > prompt))
        .then(|| text.chars().take(256).collect())
    });
    summary
}

/// Describe only recognizable command activities. Do not expose command
/// arguments or infer the task's purpose, success, or completion from tool output.
fn command_activity(command: &str) -> &'static str {
    let command = command.trim();
    if command.starts_with("&&") {
        return "명령 실행";
    }
    let mut words = command.trim_start_matches('&').split_whitespace();
    let executable = words
        .next()
        .unwrap_or_default()
        .trim_matches(['\'', '"'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let executable = executable.strip_suffix(".exe").unwrap_or(&executable);
    let next = words.next().unwrap_or_default();
    match executable {
        "get-content" | "cat" | "head" | "tail" => "파일 내용 확인",
        "rg" if next == "--files" => "파일 목록 확인",
        "rg" | "grep" | "select-string" | "findstr" => "코드·텍스트 검색",
        "get-childitem" | "ls" | "dir" | "tree" => "파일 목록 확인",
        "get-process" | "ps" | "tasklist" => "실행 중인 프로그램 확인",
        "get-ciminstance" | "get-computerinfo" => "시스템 정보 확인",
        "get-filehash" | "sha256sum" => "파일 무결성 확인",
        "cargo" => match next {
            "test" | "check" | "clippy" => "코드 검증",
            "build" => "프로그램 빌드",
            "fmt" => "코드 형식 확인",
            _ => "개발 도구 실행",
        },
        "test.ps1" | "verify-ux.ps1" | "verify-goal-hud.cjs" => "동작 검증",
        "build.ps1" => "프로그램 빌드",
        "node" if next == "--check" => "코드 검증",
        "python" | "python3" | "py" if next == "-m" && words.next() == Some("pytest") => {
            "테스트 실행"
        }
        "npm" | "pnpm" | "yarn" => match next {
            "test" => "테스트 실행",
            "build" => "프로그램 빌드",
            "run" => match words.next().unwrap_or_default() {
                "test" => "테스트 실행",
                "build" => "프로그램 빌드",
                _ => "개발 도구 실행",
            },
            _ => "개발 도구 실행",
        },
        "git" => match next {
            "status" | "diff" => "변경 내용 확인",
            "log" | "show" => "변경 이력 확인",
            _ => "버전 관리 작업",
        },
        _ => "명령 실행",
    }
}

/// Localize the known TUI status without copying arbitrary suffixes into the HUD.
pub fn activity_label(text: &str) -> String {
    let text = activity_text(text);
    let label = if text.starts_with("Thinking") {
        "검토 중"
    } else if text.starts_with("Reconnecting") {
        "연결 재시도 중"
    } else if text.starts_with("Interrupted") {
        "중단됨"
    } else if text.starts_with("Goal paused") {
        "목표 일시정지"
    } else if text.starts_with("Goal stalled") {
        "목표 중단"
    } else if text == "Resume paused goal?" {
        "목표 재개 선택 대기"
    } else if text == "Working" || text.starts_with("Working (") {
        "작업 중"
    } else {
        "상태 정보 없음"
    };
    let elapsed = text
        .split_once('(')
        .and_then(|(_, time)| time.split_once(')'))
        .map(|(time, _)| time.split(" • ").next().unwrap_or_default())
        .and_then(|time| {
            let parts: Option<Vec<String>> = time
                .split_whitespace()
                .take(4)
                .map(|part| {
                    let (number, unit) = part.split_at_checked(part.len().checked_sub(1)?)?;
                    let number: u32 = number.parse().ok()?;
                    let unit = match unit {
                        "s" => "초",
                        "m" => "분",
                        "h" => "시간",
                        _ => return None,
                    };
                    Some(format!("{number}{unit}"))
                })
                .collect();
            parts.filter(|parts| !parts.is_empty() && parts.len() <= 3)
        });
    elapsed.map_or_else(
        || label.to_owned(),
        |parts| format!("{label} · {}", parts.join(" ")),
    )
}

fn battery(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (lower.starts_with("ai battery")
        || lower.starts_with("ai-battery")
        || ((lower.contains("codex") || lower.contains("claude"))
            && text.contains(['❚', '▰', '▱', '▮', '█', '■', '□'])))
        && text.contains('%')
}

fn activity_text(text: &str) -> &str {
    text.trim_start_matches(|c: char| {
        matches!(c, '•' | '·' | '●' | '◦' | ' ') || ('\u{2800}'..='\u{28ff}').contains(&c)
    })
}

fn activity(text: &str) -> bool {
    let text = activity_text(text);
    text == "Working"
        || text.starts_with("Working (")
        || text.starts_with("Thinking (")
        || text.starts_with("Reconnecting")
        || text.starts_with("Interrupted")
        || text.starts_with("Goal paused")
        || text.starts_with("Goal stalled")
        || text == "Resume paused goal?"
}

fn chrome(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    text.is_empty()
        || lower.contains("5h limit:")
        || lower.contains("weekly limit:")
        || lower.contains("/status")
        || lower.contains("context left")
        || lower.contains("context remaining")
        || lower.contains("? for shortcuts")
        || lower.contains("esc to interrupt") && !activity(text)
        || lower.contains("ask codex")
        || lower.starts_with("openai codex")
        || lower.starts_with(">_ openai codex")
        || lower.starts_with("worked for ")
        || lower.contains("warning ·")
        || lower.contains("warning") && lower.contains("f2 to view")
}

fn plan_header(text: &str) -> bool {
    matches!(
        text.trim_start_matches(['•', '#', ' ', '─', '└'])
            .trim_end_matches(':'),
        "Updated Plan" | "Plan updated" | "Plan" | "작업 계획" | "계획"
    )
}

fn task(text: &str) -> Option<Task> {
    let text = text.trim_start_matches([' ', '└', '├', '│', '-', '•']);
    let (state, rest) = [
        ("[x]", TaskState::Completed),
        ("[X]", TaskState::Completed),
        ("[ ]", TaskState::Pending),
        ("[~]", TaskState::Running),
        ("[!]", TaskState::Stopped),
        ("✔", TaskState::Completed),
        ("✓", TaskState::Completed),
        ("☑", TaskState::Completed),
        ("□", TaskState::Pending),
        ("☐", TaskState::Pending),
        ("◻", TaskState::Pending),
        ("◐", TaskState::Running),
        ("▶", TaskState::Running),
        ("✗", TaskState::Stopped),
    ]
    .into_iter()
    .find_map(|(marker, state)| {
        text.strip_prefix(marker)
            .filter(|rest| rest.starts_with(char::is_whitespace))
            .map(|rest| (state, rest.trim_start()))
    })?;
    (!rest.trim().is_empty()).then(|| Task {
        text: rest.chars().take(256).collect(),
        state,
    })
}

fn parse_plan(rows: &[TextRow]) -> Option<Plan> {
    let mut plan = Plan {
        tasks: Vec::new(),
        complete: true,
    };
    let mut wrapped = false;
    let mut terminated = false;
    for row in rows.iter().take(128) {
        let text = if wrapped {
            row.text.as_str()
        } else if row.wrapped {
            row.text.trim_start()
        } else {
            row.text.trim()
        };
        if text.contains("more steps")
            || text.contains("steps hidden")
            || text.starts_with("+ ")
            || text.starts_with('…')
            || text.starts_with("...")
            || text.contains("단계 생략")
        {
            plan.complete = false;
            break;
        }
        if wrapped {
            if text.is_empty() || chrome(text) || activity(text) {
                break;
            }
            if let Some(item) = plan.tasks.last_mut() {
                item.text
                    .extend(text.chars().take(256 - item.text.chars().count()));
            }
            wrapped = row.wrapped;
        } else if let Some(item) = task(text) {
            if plan.tasks.len() == 64 {
                plan.complete = false;
                break;
            }
            plan.tasks.push(item);
            wrapped = row.wrapped;
        } else if !plan.tasks.is_empty() {
            terminated = true;
            break;
        } else if !text.is_empty() {
            break;
        }
    }
    if wrapped || !terminated {
        plan.complete = false;
    }
    (!plan.tasks.is_empty()).then_some(plan)
}

pub fn preview_rows<T: EventListener>(term: &Term<T>, count: usize, input: bool) -> Vec<i32> {
    if count == 0 {
        return Vec::new();
    }
    let offset = term.grid().display_offset();
    if offset > 0 {
        let end = term.screen_lines() as i32 - offset as i32;
        return (end - count.min(term.screen_lines()) as i32..end).collect();
    }
    let cursor = term
        .grid()
        .cursor
        .point
        .line
        .0
        .clamp(0, term.screen_lines() as i32 - 1);
    let mut rows = Vec::new();
    let limit = count.saturating_sub(usize::from(input));
    for line in (0..cursor).rev() {
        if rows.len() == limit {
            break;
        }
        if (0..term.columns()).any(|col| {
            let cell = &term.grid()[Line(line)][Column(col)];
            !cell.c.is_whitespace()
                && !cell
                    .flags
                    .intersects(Flags::HIDDEN | Flags::WIDE_CHAR_SPACER)
        }) {
            rows.push(line);
        }
    }
    rows.reverse();
    if input {
        rows.push(cursor);
    }
    rows
}

pub fn horizontal_start(cursor_column: usize, visible: usize, columns: usize) -> usize {
    if columns <= visible || cursor_column < visible {
        0
    } else {
        (cursor_column + 2)
            .saturating_sub(visible)
            .min(columns.saturating_sub(visible))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::{
        event::{Event, EventListener},
        term::Config,
        vte::ansi::Processor,
    };
    struct Sink;
    impl EventListener for Sink {
        fn send_event(&self, _: Event) {}
    }
    fn screen(text: &str, cols: usize, rows: usize) -> Term<Sink> {
        let mut term = Term::new(
            Config::default(),
            &crate::terminal::GridSize { cols, rows },
            Sink,
        );
        let mut processor: Processor = Processor::new();
        processor.advance(&mut term, text.as_bytes());
        term
    }
    #[test]
    fn battery_extent_uses_terminal_cells_including_indent_and_wide_glyphs() {
        let summary = summarize(&screen("  Codex ❚❚ 67% | 한글\r\n", 100, 24));
        // 17 single-column cells and two double-column Hangul cells. Font
        // fallback advances must not decide whether this VT row is clipped.
        assert_eq!(summary.battery_columns, Some(21));
        assert_eq!(summary.battery_text.as_deref(), Some("Codex ❚❚ 67% | 한글"));
    }
    #[test]
    fn summary_uses_actual_battery_below_cursor_and_filters_codex_footer() {
        let term = screen("\x1b[2;1H실제 작업 응답\x1b[19;1H• Working (3s • esc to interrupt)\x1b[23;1H5h limit: 35% left · /status\x1b[24;1HCodex \x1b[32;1m❚❚❚❚\x1b[90m❚❚\x1b[0m 67% | Claude □□ 90%\x1b[21;1H› Ask Codex to do anything", 100, 24);
        let summary = summarize(&term);
        assert_eq!(summary.activity.as_deref(), Some("Working (3s)"));
        assert_eq!(summary.recent.as_deref(), Some("실제 작업 응답"));
        assert_eq!(summary.battery_row, Some(23));
        assert!(summary.battery_text.unwrap().contains("67%"));
        assert!(term.grid()[Line(23)][Column(6)].flags.contains(Flags::BOLD));
        assert_eq!((term.columns(), term.screen_lines()), (100, 24));
    }
    #[test]
    fn summary_reads_explicit_plan_without_equating_interrupt_with_completion() {
        let term = screen("› 요약 개선\r\n• Updated Plan\r\n  └ [x] 화면 구조 분석\r\n    [~] 한글 표시 구현\r\n    [!] 외부 검증\r\n\r\n• Interrupted\r\n› Ask Codex to do anything", 100, 24);
        let summary = summarize(&term);
        assert_eq!(summary.activity.as_deref(), Some("Interrupted"));
        let plan = summary.plan.unwrap();
        assert_eq!(plan.percent(), Some(33));
        assert_eq!(plan.current().unwrap().text, "한글 표시 구현");
        assert_eq!(plan.last_completed().unwrap().text, "화면 구조 분석");
        assert_eq!(plan.tasks[2].state, TaskState::Stopped);
    }
    #[test]
    fn summary_hides_percentage_for_collapsed_and_cut_off_plans() {
        for text in [
            "Plan\r\n[x] 검사 완료\r\n… 2 more steps\r\n",
            "\x1b[23;1HPlan\r\n[x] 마지막 화면 줄",
        ] {
            let summary = summarize(&screen(text, 100, 24));
            let plan = summary.plan.unwrap();
            assert_eq!(plan.completed(), 1);
            assert_eq!(plan.percent(), None);
        }
    }
    #[test]
    fn summary_joins_wrapped_hangul_task_names() {
        let summary = summarize(&screen(
            "작업 계획\r\n[~] 한글로 길게 표시하는 작업 이름 확인\r\n[ ] 검증\r\n\r\n",
            24,
            24,
        ));
        let plan = summary.plan.unwrap();
        assert_eq!(plan.tasks.len(), 2);
        assert_eq!(
            plan.current().unwrap().text,
            "한글로 길게 표시하는 작업 이름 확인"
        );
        assert_eq!(plan.percent(), Some(0));
    }
    #[test]
    fn new_submission_drops_old_plan_activity_and_tool_summary() {
        let summary = summarize(&screen("› 첫 요청\r\nPlan\r\n[x] 이전 작업\r\n\r\n• Ran printf old\r\n• Interrupted\r\n› 새 요청\r\n• Working (0s)\r\n› Ask Codex to do anything",100,24));
        assert!(summary.plan.is_none());
        assert!(summary.operation.is_none());
        assert!(summary.recent.is_none());
        assert_eq!(summary.activity.as_deref(), Some("Working (0s)"));
    }
    #[test]
    fn code_and_ordinary_checklists_are_not_work_progress() {
        for text in [
            "[x] 일반 목록\r\n[ ] 다음 항목\r\n",
            "```text\r\nPlan\r\n[x] 코드 예시\r\n```\r\n",
        ] {
            assert!(summarize(&screen(text, 100, 24)).plan.is_none());
        }
    }
    #[test]
    fn response_end_and_quota_alone_do_not_create_progress_or_battery() {
        let summary = summarize(&screen("• Ran cargo check\r\n확인 결과\r\nWorked for 5s\r\n5h limit: 35% left · /status\r\n› Ask Codex to do anything",100,24));
        assert!(summary.plan.is_none());
        assert!(summary.battery_row.is_none());
        assert!(summary.activity.is_none());
        assert_eq!(summary.operation.as_deref(), Some("최근 활동: 코드 검증"));
        assert_eq!(summary.recent.as_deref(), Some("확인 결과"));
    }
    #[test]
    fn operations_describe_activity_without_exposing_internal_arguments() {
        for (command, label) in [
            ("Get-Content src/app.rs -Encoding UTF8", "파일 내용 확인"),
            ("& scripts/build.ps1", "프로그램 빌드"),
            ("rg --files src", "파일 목록 확인"),
            ("rg 'private-pattern' src", "코드·텍스트 검색"),
            ("cargo test --locked", "코드 검증"),
            ("npm run build", "프로그램 빌드"),
            ("python -m pytest", "테스트 실행"),
            ("git diff -- private-file", "변경 내용 확인"),
            ("printf '%s' 'cargo test'", "명령 실행"),
            (
                "$taskSourceRoot=Join-Path (Get-Location).Path 'private-path'",
                "명령 실행",
            ),
            ("unknown --token=SECRET", "명령 실행"),
        ] {
            let summary = summarize(&screen(
                &format!("• Ran {command}\r\n› Ask Codex to do anything"),
                160,
                24,
            ));
            assert_eq!(summary.operation, Some(format!("최근 활동: {label}")));
            assert!(summary.plan.is_none());
        }
    }
    #[test]
    fn statuses_use_readable_labels_and_valid_elapsed_time_only() {
        for (source, expected) in [
            ("Working (58s)", "작업 중 · 58초"),
            ("Thinking (1m 24s)", "검토 중 · 1분 24초"),
            ("Working (3s • esc to interrupt)", "작업 중 · 3초"),
            ("Working (internal-command)", "작업 중"),
            ("Interrupted", "중단됨"),
            ("Goal paused", "목표 일시정지"),
            ("Resume paused goal?", "목표 재개 선택 대기"),
            ("unknown status", "상태 정보 없음"),
        ] {
            assert_eq!(activity_label(source), expected);
        }
    }
    #[test]
    fn summary_skips_blank_rows_and_input_follows_cursor_without_resizing_the_tui() {
        let mut term = Term::new(
            Config::default(),
            &crate::terminal::GridSize { cols: 80, rows: 24 },
            Sink,
        );
        let mut processor: Processor = Processor::new();
        processor.advance(
            &mut term,
            b"\x1b[3;1Houtput\x1b[20;1Hworking\x1b[24;1H> typing",
        );
        assert_eq!(preview_rows(&term, 2, true), vec![19, 23]);
        assert_eq!(preview_rows(&term, 1, false), vec![19]);
        assert!(preview_rows(&term, 0, true).is_empty());
        assert_eq!((term.columns(), term.screen_lines()), (80, 24));
        assert_eq!(horizontal_start(79, 30, 80), 50);
        assert_eq!(horizontal_start(8, 30, 80), 0);
    }

    #[test]
    fn compact_input_scrolls_output_without_moving_the_live_cursor() {
        let mut term = screen("", 40, 4);
        let mut processor: Processor = Processor::new();
        for line in 0..20 {
            processor.advance(&mut term, format!("line-{line:02}\r\n").as_bytes());
        }
        let cursor = term.grid().cursor.point;
        let live_rows = preview_rows(&term, 2, true);
        term.scroll_display(alacritty_terminal::grid::Scroll::Delta(6));
        let rows = preview_rows(&term, 2, true);
        let text: String = (0..term.columns())
            .map(|col| term.grid()[Line(rows[0])][Column(col)].c)
            .collect();
        assert!(text.starts_with("line-13"), "{text}");
        assert_eq!(term.grid().cursor.point, cursor);
        term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
        assert_eq!(preview_rows(&term, 2, true), live_rows);
    }
}
