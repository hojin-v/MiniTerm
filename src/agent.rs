//! Official agent events only. Payloads are reduced before crossing IPC;
//! prompts, assistant messages and transcript paths never enter the broker.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agent {
    Codex,
    Claude,
}
impl Agent {
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Unknown,
    Running,
    Complete,
    NeedsInput,
    Failed,
    Interrupted,
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "연결됨 · 상태 미확인",
            Self::Running => "응답 중",
            Self::Complete => "응답 완료",
            Self::NeedsInput => "입력 필요",
            Self::Failed => "응답 오류",
            Self::Interrupted => "중단됨",
        }
    }
    pub fn alerts(self) -> bool {
        matches!(self, Self::Complete | Self::NeedsInput | Self::Failed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentEvent {
    pub session_id: String,
    pub agent: Agent,
    pub agent_session_id: String,
    pub event_id: Option<String>,
    pub state: State,
    pub cwd: String,
    pub occurred_at_ms: u64,
}
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn text(value: &serde_json::Value, key: &str, max: usize) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect()
}

pub fn normalize(agent: Agent, session_id: &str, raw: &[u8]) -> Result<Option<AgentEvent>, String> {
    if uuid::Uuid::parse_str(session_id).is_err() {
        return Err("MiniTerm 세션 ID가 없습니다.".into());
    }
    if raw.len() > 2 * 1024 * 1024 {
        return Err("에이전트 이벤트가 너무 큽니다.".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(raw).map_err(|_| "에이전트 이벤트 JSON을 확인하세요.")?;
    let (state, id_key, event_key) = match agent {
        Agent::Codex => {
            if value.get("type").and_then(|v| v.as_str()) != Some("agent-turn-complete") {
                return Ok(None);
            }
            (State::Complete, "thread-id", "turn-id")
        }
        Agent::Claude => {
            // Common agent_id denotes a subagent, even for a normally main-session event.
            if value
                .get("agent_id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| !id.is_empty())
            {
                return Ok(None);
            }
            let state = match value
                .get("hook_event_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
            {
                "SessionStart" => State::Unknown,
                "UserPromptSubmit" | "PostToolUse" => State::Running,
                "Stop" if value.get("stop_hook_active").and_then(|v| v.as_bool()) == Some(true) => {
                    State::Unknown
                }
                "Stop" => State::Complete,
                "StopFailure" => State::Failed,
                "PermissionRequest" => State::NeedsInput,
                "Notification" => match value.get("notification_type").and_then(|v| v.as_str()) {
                    Some("permission_prompt" | "idle_prompt" | "elicitation_dialog") => {
                        State::NeedsInput
                    }
                    _ => return Ok(None),
                },
                "SessionEnd" => State::Unknown,
                _ => return Ok(None),
            };
            (state, "session_id", "event_id")
        }
    };
    let event_id = text(&value, event_key, 128);
    let event = AgentEvent {
        session_id: session_id.into(),
        agent,
        agent_session_id: text(&value, id_key, 128),
        event_id: (!event_id.is_empty()).then_some(event_id),
        state,
        cwd: text(&value, "cwd", 2048),
        occurred_at_ms: now_ms(),
    };
    event.validate()?;
    Ok(Some(event))
}
impl AgentEvent {
    pub fn validate(&self) -> Result<(), String> {
        if uuid::Uuid::parse_str(&self.session_id).is_err()
            || self.agent_session_id.is_empty()
            || self.agent_session_id.len() > 512
            || self.cwd.len() > 8192
            || self
                .event_id
                .as_ref()
                .is_some_and(|id| id.is_empty() || id.len() > 512)
            || self
                .agent_session_id
                .chars()
                .chain(self.cwd.chars())
                .any(char::is_control)
        {
            return Err("에이전트 이벤트 식별자를 확인하세요.".into());
        }
        Ok(())
    }
    pub fn project(&self) -> String {
        self.cwd
            .trim_end_matches(['/', '\\'])
            .rsplit(['/', '\\'])
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("터미널")
            .chars()
            .take(64)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct SessionStatus {
    pub agent: Option<Agent>,
    pub state: State,
    pub unread: bool,
    pub alive: bool,
    pub exited: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub progress: crate::progress::Slot,
}
impl Default for SessionStatus {
    fn default() -> Self {
        Self {
            agent: None,
            state: State::Unknown,
            unread: false,
            alive: true,
            exited: None,
            progress: Default::default(),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub occurred_at_ms: u64,
}

#[derive(Default)]
pub struct Broker {
    pub sessions: HashMap<String, SessionStatus>,
    pub notices: VecDeque<Notice>,
    pub selected: Option<String>,
    seen: VecDeque<(String, u64)>,
}
impl Broker {
    pub fn progress(
        &mut self,
        session_id: &str,
        update: crate::progress::Update,
    ) -> Result<Option<crate::progress::Record>, String> {
        let session = self
            .sessions
            .get_mut(session_id)
            .filter(|session| {
                session.alive
                    && !session
                        .exited
                        .as_ref()
                        .is_some_and(|exit| exit.load(std::sync::atomic::Ordering::Acquire))
            })
            .ok_or("이 세션은 이미 종료되었습니다.")?;
        session.progress.apply(update)
    }
    pub fn register(&mut self, id: String) {
        self.sessions.entry(id).or_default();
    }
    pub fn remove(&mut self, id: &str) {
        self.sessions.remove(id);
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
    }
    pub fn read(&mut self, id: &str) {
        if let Some(s) = self.sessions.get_mut(id) {
            s.unread = false;
        }
    }
    /// Unknown/stale session IDs never select a newly created process or emit a toast.
    pub fn receive(&mut self, event: AgentEvent, observed: bool) -> Option<Notice> {
        if event.validate().is_err() {
            return None;
        }
        let session = self.sessions.get_mut(&event.session_id)?;
        if !session.alive
            || session
                .exited
                .as_ref()
                .is_some_and(|exit| exit.load(std::sync::atomic::Ordering::Acquire))
        {
            return None;
        }
        let exact = event.event_id.is_some();
        let key = serde_json::to_string(&(
            &event.session_id,
            event.agent,
            &event.agent_session_id,
            event.state,
            &event.event_id,
        ))
        .ok()?;
        let now = now_ms();
        if self.seen.iter().any(|(previous, at)| {
            previous == &key
                && (exact || (session.state == event.state && now.saturating_sub(*at) < 2000))
        }) {
            return None;
        }
        self.seen.push_back((key, now));
        while self.seen.len() > 512 {
            self.seen.pop_front();
        }
        session.agent = Some(event.agent);
        session.state = event.state;
        session.unread = event.state.alerts() && !observed;
        if !event.state.alerts() {
            return None;
        }
        let notice = Notice {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: event.session_id.clone(),
            title: format!(
                "{} {} · {}",
                event.agent.label(),
                event.state.label(),
                event.project()
            ),
            occurred_at_ms: now,
        };
        self.notices.push_front(notice.clone());
        self.notices.truncate(100);
        (!observed).then_some(notice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "71a461b4-1259-4ac7-b321-243d742c54b4";
    fn event(turn: &str) -> AgentEvent {
        normalize(Agent::Codex, ID, format!(r#"{{"type":"agent-turn-complete","thread-id":"thread1","turn-id":"{turn}","cwd":"C:/work/app","last-assistant-message":"SECRET"}}"#).as_bytes()).unwrap().unwrap()
    }
    #[test]
    fn official_events_do_not_leak_content_or_complete_subagents() {
        let json = serde_json::to_string(&event("1")).unwrap();
        assert!(!json.contains("SECRET"));
        assert!(
            normalize(Agent::Codex, ID, br#"{"type":"approval-requested"}"#)
                .unwrap()
                .is_none()
        );
        for raw in [
            br#"{"hook_event_name":"SubagentStop","session_id":"a"}"#.as_slice(),
            br#"{"hook_event_name":"Stop","session_id":"a","agent_id":"child"}"#,
        ] {
            assert!(normalize(Agent::Claude, ID, raw).unwrap().is_none());
        }
        let retry = normalize(
            Agent::Claude,
            ID,
            br#"{"hook_event_name":"Stop","session_id":"a","stop_hook_active":true}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(retry.state, State::Unknown);
    }
    #[test]
    fn ui_registration_preserves_events_received_during_shell_startup() {
        let mut broker = Broker::default();
        broker.register(ID.into());
        assert!(broker.receive(event("early"), false).is_some());
        broker.register(ID.into());
        assert_eq!(broker.sessions[ID].state, State::Complete);
        assert!(broker.sessions[ID].unread);
        assert!(broker.receive(event("early"), false).is_none());
    }
    #[test]
    fn duplicate_turns_and_stale_sessions_never_alert_again() {
        let mut b = Broker::default();
        b.register(ID.into());
        assert!(b.receive(event("1"), false).is_some());
        assert!(b.receive(event("1"), false).is_none());
        assert!(b.receive(event("2"), true).is_none());
        assert_eq!(b.notices.len(), 2);
        assert!(!b.sessions[ID].unread);
        b.remove(ID);
        assert!(b.receive(event("3"), false).is_none());
    }
    #[test]
    fn task_reports_are_session_scoped_and_turn_completion_does_not_finish_plan() {
        use crate::mini::{Task, TaskState};
        use crate::progress::{Report, Update, WorkState};
        let report = || Update::Begin {
            report: Report {
                title: "입력 검증".into(),
                state: WorkState::Running,
                tasks: vec![Task {
                    text: "한글 입력 검증".into(),
                    state: TaskState::Running,
                }],
            },
        };
        let mut broker = Broker::default();
        assert!(broker.progress(ID, report()).is_err());
        broker.register(ID.into());
        let record = broker.progress(ID, report()).unwrap();
        broker.register(ID.into());
        broker.receive(event("response-end"), true);
        assert_eq!(broker.sessions[ID].progress.record(), record);
        assert_eq!(broker.sessions[ID].state, State::Complete);
        let exited = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        broker.sessions.get_mut(ID).unwrap().exited = Some(exited);
        assert!(broker.progress(ID, report()).is_err());
        broker.remove(ID);
        assert!(broker.progress(ID, Update::Read).is_err());
    }
    #[test]
    fn claude_resumes_after_permission_and_errors_are_not_success() {
        let mut b = Broker::default();
        b.register(ID.into());
        for (name, state) in [
            ("UserPromptSubmit", State::Running),
            ("PermissionRequest", State::NeedsInput),
            ("PostToolUse", State::Running),
            ("StopFailure", State::Failed),
        ] {
            let raw = format!(r#"{{"hook_event_name":"{name}","session_id":"a"}}"#);
            b.receive(
                normalize(Agent::Claude, ID, raw.as_bytes())
                    .unwrap()
                    .unwrap(),
                false,
            );
            assert_eq!(b.sessions[ID].state, state);
        }
        assert_eq!(b.notices.len(), 2);
    }
}
