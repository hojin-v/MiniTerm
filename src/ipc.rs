//! A local-only, current-user named pipe with bounded frames and cancellable I/O.
use crate::agent::AgentEvent;
use serde::{Deserialize, Serialize};
use std::{
    io,
    mem::{size_of, zeroed},
    sync::Arc,
    thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, HANDLE,
        INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
    },
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    },
    Storage::FileSystem::{
        CreateFileW, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
        OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
    },
    System::{
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeServerProcessId, WaitNamedPipeW,
            PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
        },
        Threading::{
            CreateEventW, CreateMutexW, GetCurrentProcess, OpenMutexW, OpenProcessToken, SetEvent,
            WaitForMultipleObjects,
        },
        IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
    },
};

const MAX_FRAME: usize = 64 * 1024;
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Event {
        event: AgentEvent,
    },
    CodexEvent {
        connection: String,
        event: AgentEvent,
    },
    Activate {
        session_id: String,
    },
    Show,
    Sessions,
    Diagnostic {
        command: String,
    },
    Progress {
        session_id: String,
        update: crate::progress::Update,
    },
    PrepareCodex {
        session_id: String,
        home: std::path::PathBuf,
        cwd: std::path::PathBuf,
        environment: std::collections::BTreeMap<String, String>,
    },
    CodexPrepared {
        session_id: String,
        ticket: String,
    },
    CloseCodexLink {
        session_id: String,
        connection: String,
    },
}
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preparation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_connection: Option<String>,
    pub ok: bool,
    pub sessions: Vec<String>,
    pub error: Option<String>,
    pub details: Option<Snapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<crate::progress::Record>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub helper_capabilities: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub states: std::collections::HashMap<String, crate::agent::State>,
    pub unread: Vec<String>,
    pub notice_count: usize,
    pub toast_count: u64,
    pub toast_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<std::collections::BTreeMap<String, [f32; 4]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixture_sample_ready: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_scroll: Option<TerminalScroll>,
}
/// Buffer metadata only, available in an isolated diagnostic instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalScroll {
    pub win32_input: bool,
    pub alternate_screen: bool,
    pub mouse_reporting: bool,
    pub history_lines: usize,
    pub display_offset: usize,
    pub wheel_events: usize,
    pub last_wheel_delta: f32,
    pub last_wheel_hovered: bool,
    pub last_wheel_typed: bool,
    pub focused: bool,
    pub composing: bool,
    pub enter_events: usize,
    pub enter_encoded: usize,
    pub last_enter_win32: bool,
    pub last_enter_modifiers: [bool; 3],
    pub last_text_frame: u64,
    pub last_enter_frame: u64,
    pub codex_ready: bool,
    pub codex_writer_blocked: bool,
}
impl Reply {
    pub fn ok() -> Self {
        Self {
            ok: true,
            ..Self::default()
        }
    }
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            ..Self::default()
        }
    }
}
struct Handle(HANDLE);
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn handle(raw: HANDLE) -> io::Result<Handle> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(Handle(raw))
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

pub fn user_sid() -> io::Result<String> {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = Handle(token);
        let mut count = 0;
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut count);
        if count == 0 || count > 65536 {
            return Err(io::Error::last_os_error());
        }
        // u64 allocation ensures TOKEN_USER and its embedded SID are correctly aligned.
        let mut buffer = vec![0u64; (count as usize).div_ceil(8)];
        if GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            count,
            &mut count,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let user = &*(buffer.as_ptr().cast::<TOKEN_USER>());
        let mut result = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut result) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut len = 0;
        while *result.add(len) != 0 {
            len += 1;
        }
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(result, len));
        LocalFree(result.cast());
        Ok(sid)
    }
}
fn name() -> io::Result<Vec<u16>> {
    Ok(wide(&format!(r"\\.\pipe\MiniTerm.{}", instance_key()?)))
}

fn instance_key() -> io::Result<String> {
    let sid = user_sid()?;
    let Ok(id) = std::env::var("MINITERM_INSTANCE_ID") else {
        return Ok(sid);
    };
    if std::env::var("MINITERM_DIAGNOSTICS").as_deref() != Ok("1")
        || std::env::var_os("MINITERM_DATA_DIR").is_none()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "별도 인스턴스는 격리된 진단 환경에서만 사용할 수 있습니다.",
        ));
    }
    let id = uuid::Uuid::parse_str(&id).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "진단 인스턴스 ID를 확인하세요.",
        )
    })?;
    Ok(format!("{sid}.diagnostic.{id}"))
}

pub struct Server {
    stop: Arc<Handle>,
    _worker: thread::JoinHandle<()>,
    _instance: Arc<Handle>,
}
impl Drop for Server {
    fn drop(&mut self) {
        unsafe {
            SetEvent(self.stop.0);
        }
    }
}
pub fn serve(callback: impl Fn(Request) -> Reply + Send + Sync + 'static) -> io::Result<Server> {
    let instance_name = wide(&format!(r"Local\MiniTerm.{}", instance_key()?));
    let instance = handle(unsafe { CreateMutexW(std::ptr::null(), 0, instance_name.as_ptr()) })?;
    if unsafe { GetLastError() } == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "MiniTerm이 이미 실행 중입니다.",
        ));
    }
    let instance = Arc::new(instance);
    let worker_instance = instance.clone();
    let name = name()?;
    let sid = user_sid()?;
    let pipe = create_pipe(&name, &sid)?;
    let stop = Arc::new(handle(unsafe {
        CreateEventW(std::ptr::null(), 1, 0, std::ptr::null())
    })?);
    let thread_stop = stop.clone();
    let worker = thread::spawn(move || {
        let _instance = worker_instance;
        let mut pipe = pipe;
        loop {
            if connected(&pipe, &thread_stop).is_err() {
                break;
            }
            let reply = read_frame(&pipe, &thread_stop)
                .and_then(|data| serde_json::from_slice::<Request>(&data).map_err(io::Error::other))
                .map(callback_ref(&callback))
                .unwrap_or_else(|_| Reply::error("잘못된 IPC 요청입니다."));
            if let Ok(data) = serde_json::to_vec(&reply) {
                let _ = write_frame(&pipe, &thread_stop, &data);
            }
            // Closing disconnects without FlushFileBuffers, which could block on a stalled client.
            drop(pipe);
            if unsafe {
                windows_sys::Win32::System::Threading::WaitForSingleObject(thread_stop.0, 0)
            } == WAIT_OBJECT_0
            {
                break;
            }
            match create_pipe(&name, &sid) {
                Ok(next) => pipe = next,
                Err(_) => break,
            }
        }
    });
    Ok(Server {
        stop,
        _worker: worker,
        _instance: instance,
    })
}
/// This does not depend on a responsive GUI or IPC client queue.
pub fn running() -> bool {
    let Ok(sid) = instance_key() else {
        return true;
    };
    let name = wide(&format!(r"Local\MiniTerm.{sid}"));
    match handle(unsafe { OpenMutexW(0x00100000, 0, name.as_ptr()) }) {
        Ok(_handle) => true,
        Err(error) => {
            error.raw_os_error()
                != Some(windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND as i32)
        }
    }
}
fn callback_ref<'a>(callback: &'a impl Fn(Request) -> Reply) -> impl FnOnce(Request) -> Reply + 'a {
    callback
}

fn create_pipe(name: &[u16], sid: &str) -> io::Result<Handle> {
    unsafe {
        let sddl = wide(&format!("D:P(A;;GA;;;{sid})(A;;GA;;;SY)"));
        let mut descriptor = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let pipe = CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            MAX_FRAME as u32,
            MAX_FRAME as u32,
            1000,
            &attributes,
        );
        let result = handle(pipe);
        LocalFree(descriptor);
        result
    }
}
fn overlapped() -> io::Result<(Handle, OVERLAPPED)> {
    let event = handle(unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) })?;
    let mut state: OVERLAPPED = unsafe { zeroed() };
    state.hEvent = event.0;
    Ok((event, state))
}
fn finish(pipe: &Handle, stop: &Handle, state: &mut OVERLAPPED, timeout: u32) -> io::Result<u32> {
    unsafe {
        let events = [state.hEvent, stop.0];
        let wait = WaitForMultipleObjects(2, events.as_ptr(), 0, timeout);
        if wait != WAIT_OBJECT_0 {
            CancelIoEx(pipe.0, state);
            let mut bytes = 0;
            // Cancellation must finish before the OVERLAPPED and buffer leave scope.
            GetOverlappedResult(pipe.0, state, &mut bytes, 1);
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "IPC 요청 시간이 초과되었습니다.",
            ));
        }
        let mut bytes = 0;
        if GetOverlappedResult(pipe.0, state, &mut bytes, 0) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(bytes)
    }
}
fn connected(pipe: &Handle, stop: &Handle) -> io::Result<()> {
    let (_event, mut state) = overlapped()?;
    if unsafe { ConnectNamedPipe(pipe.0, &mut state) } != 0 {
        return Ok(());
    }
    match unsafe { GetLastError() } {
        ERROR_PIPE_CONNECTED => Ok(()),
        ERROR_IO_PENDING => finish(pipe, stop, &mut state, u32::MAX).map(|_| ()),
        _ => Err(io::Error::last_os_error()),
    }
}
fn transfer(pipe: &Handle, stop: &Handle, buffer: &mut [u8], write: bool) -> io::Result<()> {
    let mut offset = 0;
    while offset < buffer.len() {
        let (_event, mut state) = overlapped()?;
        let mut bytes = 0;
        let result = unsafe {
            if write {
                WriteFile(
                    pipe.0,
                    buffer[offset..].as_ptr().cast(),
                    (buffer.len() - offset) as u32,
                    &mut bytes,
                    &mut state,
                )
            } else {
                ReadFile(
                    pipe.0,
                    buffer[offset..].as_mut_ptr().cast(),
                    (buffer.len() - offset) as u32,
                    &mut bytes,
                    &mut state,
                )
            }
        };
        if result == 0 {
            if unsafe { GetLastError() } != ERROR_IO_PENDING {
                return Err(io::Error::last_os_error());
            }
            bytes = finish(pipe, stop, &mut state, 2000)?;
        }
        if bytes == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        offset += bytes as usize;
    }
    Ok(())
}
fn read_frame(pipe: &Handle, stop: &Handle) -> io::Result<Vec<u8>> {
    let mut length = [0; 4];
    transfer(pipe, stop, &mut length, false)?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "IPC 크기 제한"));
    }
    let mut data = vec![0; length];
    transfer(pipe, stop, &mut data, false)?;
    Ok(data)
}
fn write_frame(pipe: &Handle, stop: &Handle, data: &[u8]) -> io::Result<()> {
    if data.is_empty() || data.len() > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "IPC 크기 제한"));
    }
    transfer(pipe, stop, &mut (data.len() as u32).to_le_bytes(), true)?;
    transfer(pipe, stop, &mut data.to_vec(), true)
}
pub fn send(request: &Request) -> io::Result<Reply> {
    send_with_retry(request, true)
}
/// Startup must not pay a connection timeout when no other instance exists.
pub fn probe(request: &Request) -> io::Result<Reply> {
    send_with_retry(request, false)
}
fn send_with_retry(request: &Request, retry_missing: bool) -> io::Result<Reply> {
    let name = name()?;
    let start = std::time::Instant::now();
    let pipe = loop {
        let pipe = handle(unsafe {
            CreateFileW(
                name.as_ptr(),
                0xC0000000,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                std::ptr::null_mut(),
            )
        });
        match pipe {
            Ok(pipe) => break pipe,
            Err(error) => {
                if !retry_missing && error.raw_os_error() != Some(231) {
                    return Err(error);
                }
                if start.elapsed() > Duration::from_millis(1500) {
                    return Err(error);
                }
                unsafe {
                    WaitNamedPipeW(name.as_ptr(), 100);
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    };
    let stop = handle(unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) })?;
    let data = serde_json::to_vec(request).map_err(io::Error::other)?;
    if matches!(request, Request::Show | Request::Activate { .. }) {
        let mut process = 0;
        unsafe {
            if GetNamedPipeServerProcessId(pipe.0, &mut process) != 0 {
                windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(process);
            }
        }
    }
    write_frame(&pipe, &stop, &data)?;
    serde_json::from_slice(&read_frame(&pipe, &stop)?).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Uses the current user's real named pipe; app must be closed"]
    fn real_pipe_roundtrip_and_single_instance() {
        let server = serve(|_| Reply::ok()).unwrap();
        assert!(serve(|_| Reply::ok()).is_err());
        for _ in 0..10 {
            assert!(send(&Request::Sessions).unwrap().ok);
        }
        drop(server);
    }
}
