//! Explicit, bounded task reports. No model calls, transcript reads or polling.
use crate::mini::{Task, TaskState};
use serde::{Deserialize, Serialize};

pub const MAX_BYTES: usize = 48 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkState {
    Running,
    Completed,
    Paused,
    Failed,
}
impl WorkState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "진행 중",
            Self::Completed => "계획 완료",
            Self::Paused => "일시 중단",
            Self::Failed => "작업 오류",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub title: String,
    pub state: WorkState,
    pub tasks: Vec<Task>,
}
impl Report {
    pub fn validate(&self) -> Result<(), String> {
        let valid_text = |text: &str, max| {
            !text.trim().is_empty()
                && text.chars().count() <= max
                && !text.chars().any(char::is_control)
        };
        if !valid_text(&self.title, 128)
            || self.tasks.is_empty()
            || self.tasks.len() > 64
            || self.tasks.iter().any(|task| !valid_text(&task.text, 256))
            || serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > MAX_BYTES)
        {
            return Err("작업명 128자, 항목 1~64개·각 256자, JSON 48KiB 이내로 보고하세요.".into());
        }
        if self.state == WorkState::Completed
            && self
                .tasks
                .iter()
                .any(|task| task.state != TaskState::Completed)
        {
            return Err("완료 보고에는 미완료 계획 항목이 남아 있을 수 없습니다.".into());
        }
        Ok(())
    }
    pub fn from_bytes(raw: &[u8]) -> Result<Self, String> {
        if raw.len() > MAX_BYTES {
            return Err("작업 보고 JSON이 48KiB를 초과했습니다.".into());
        }
        let report: Self =
            serde_json::from_slice(raw).map_err(|_| "UTF-8 작업 보고 JSON 형식을 확인하세요.")?;
        report.validate()?;
        Ok(report)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub work_id: String,
    pub revision: u64,
    pub updated_at_ms: u64,
    pub report: Report,
}
impl Record {
    pub fn description(&self) -> String {
        let age = crate::agent::now_ms().saturating_sub(self.updated_at_ms) / 1000;
        let age = if age < 60 {
            format!("{age}초 전")
        } else {
            format!("{}분 전", age / 60)
        };
        let plan = crate::mini::Plan {
            tasks: self.report.tasks.clone(),
            complete: true,
        };
        let description = plan.description();
        let tasks = description.split_once('\n').map_or("", |(_, tasks)| tasks);
        format!(
            "{}\n에이전트 보고 · {} · {age}\n완료 항목 비율이며 소요 시간 예측이 아닙니다.\n{tasks}",
            self.report.title,
            self.report.state.label(),
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Update {
    Begin {
        report: Report,
    },
    Update {
        work_id: String,
        revision: u64,
        report: Report,
    },
    Clear {
        work_id: String,
        revision: u64,
    },
    Read,
}

#[derive(Debug, Clone, Default)]
pub struct Slot(Option<Record>);
impl Slot {
    pub fn record(&self) -> Option<Record> {
        self.0.clone()
    }
    /// A submitted user input expires the receipt. Late updates cannot restore it.
    pub fn invalidate(&mut self) {
        self.0 = None;
    }
    pub fn apply(&mut self, update: Update) -> Result<Option<Record>, String> {
        match update {
            Update::Read => Ok(self.record()),
            Update::Begin { report } => {
                report.validate()?;
                let record = Record {
                    work_id: uuid::Uuid::new_v4().to_string(),
                    revision: 1,
                    updated_at_ms: crate::agent::now_ms(),
                    report,
                };
                self.0 = Some(record.clone());
                Ok(Some(record))
            }
            Update::Update {
                work_id,
                revision,
                report,
            } => {
                report.validate()?;
                self.check_receipt(&work_id, revision)?;
                let record = Record {
                    work_id,
                    revision,
                    report,
                    updated_at_ms: crate::agent::now_ms(),
                };
                self.0 = Some(record.clone());
                Ok(Some(record))
            }
            Update::Clear { work_id, revision } => {
                self.check_receipt(&work_id, revision)?;
                self.invalidate();
                Ok(None)
            }
        }
    }
    fn check_receipt(&self, work_id: &str, revision: u64) -> Result<(), String> {
        let valid = self.0.as_ref().is_some_and(|record| {
            record.work_id == work_id && record.revision.checked_add(1) == Some(revision)
        });
        if valid {
            Ok(())
        } else {
            Err(
                "작업 보고가 만료되었거나 순서가 맞지 않습니다. 현재 작업을 다시 begin 하세요."
                    .into(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report() -> Report {
        Report {
            title: "요약 기능 수정".into(),
            state: WorkState::Running,
            tasks: vec![
                Task {
                    text: "화면 확인".into(),
                    state: TaskState::Completed,
                },
                Task {
                    text: "한글 입력 검증".into(),
                    state: TaskState::Running,
                },
                Task {
                    text: "결과 검토".into(),
                    state: TaskState::Pending,
                },
            ],
        }
    }
    #[test]
    fn explicit_reports_override_screen_plan_and_keep_actual_battery() {
        let mut slot = Slot::default();
        let record = slot.apply(Update::Begin { report: report() }).unwrap();
        let summary = crate::mini::Summary {
            battery_row: Some(22),
            battery_text: Some("Codex ❚ 67%".into()),
            activity: Some("Working".into()),
            ..Default::default()
        }
        .with_report(record);
        assert_eq!(summary.plan.as_ref().unwrap().percent(), Some(33));
        assert_eq!(
            summary.plan.as_ref().unwrap().current().unwrap().text,
            "한글 입력 검증"
        );
        assert_eq!(summary.battery_row, Some(22));
        assert_eq!(summary.activity.as_deref(), Some("Working"));
        assert!(summary
            .reported
            .unwrap()
            .description()
            .contains("에이전트 보고"));
    }
    #[test]
    fn late_duplicate_and_previous_work_updates_cannot_replace_current_plan() {
        let mut slot = Slot::default();
        let first = slot
            .apply(Update::Begin { report: report() })
            .unwrap()
            .unwrap();
        let mut changed = report();
        changed.tasks[1].state = TaskState::Completed;
        let update = |revision| Update::Update {
            work_id: first.work_id.clone(),
            revision,
            report: changed.clone(),
        };
        assert!(slot.apply(update(4)).is_err());
        assert!(slot.apply(update(2)).is_ok());
        assert!(slot.apply(update(2)).is_err());
        assert_eq!(
            slot.record().unwrap().report.tasks[1].state,
            TaskState::Completed
        );
        let next = slot
            .apply(Update::Begin { report: report() })
            .unwrap()
            .unwrap();
        assert_ne!(first.work_id, next.work_id);
        assert!(slot.apply(update(3)).is_err());
        assert_eq!(slot.record().unwrap(), next);
        slot.invalidate();
        assert!(slot.record().is_none());
        assert!(slot
            .apply(Update::Update {
                work_id: next.work_id,
                revision: 2,
                report: report()
            })
            .is_err());
    }
    #[test]
    fn completion_requires_explicit_finished_tasks_and_failure_keeps_ratio() {
        let mut input = report();
        input.state = WorkState::Completed;
        assert!(input.validate().is_err());
        input.state = WorkState::Failed;
        input.tasks[1].state = TaskState::Stopped;
        assert!(input.validate().is_ok());
        let mut slot = Slot::default();
        let summary = crate::mini::Summary::default()
            .with_report(slot.apply(Update::Begin { report: input }).unwrap());
        assert_eq!(summary.plan.unwrap().percent(), Some(33));
    }
    #[test]
    fn malformed_oversized_and_control_text_reports_are_rejected_without_mutation() {
        let mut slot = Slot::default();
        let original = slot.apply(Update::Begin { report: report() }).unwrap();
        for invalid in [
            br#"{"title":"x","state":"running","tasks":[],"prompt":"SECRET"}"#.as_slice(),
            br#"{"title":"x","state":"running","tasks":[{"text":"x","state":"unknown"}]}"#,
        ] {
            assert!(Report::from_bytes(invalid).is_err());
        }
        assert!(Report::from_bytes(&vec![b' '; MAX_BYTES + 1]).is_err());
        let mut input = report();
        input.tasks[0].text = "bad\u{1b}[31m".into();
        assert!(slot.apply(Update::Begin { report: input }).is_err());
        let mut input = report();
        input.tasks = vec![input.tasks[0].clone(); 65];
        assert!(slot.apply(Update::Begin { report: input }).is_err());
        assert_eq!(slot.record(), original);
    }
}
