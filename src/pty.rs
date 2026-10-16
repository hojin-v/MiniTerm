pub use crate::console_guard::ConsoleHost;
use crate::{native::ProcessJob, shell::ShellProfile};
use portable_pty::{native_pty_system, Child, MasterPty, PtySize};
use std::io::{Read, Write};

pub struct OpenPty {
    // Drop this first so ClosePseudoConsole never waits for an unowned host.
    pub console: ConsoleHost,
    pub master: Box<dyn MasterPty + Send>,
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub job: ProcessJob,
}

pub fn size(cols: u16, rows: u16) -> Result<PtySize, String> {
    if !(2..=1000).contains(&cols) || !(1..=500).contains(&rows) {
        return Err("터미널 크기가 유효하지 않습니다.".into());
    }
    Ok(PtySize {
        cols,
        rows,
        pixel_width: 0,
        pixel_height: 0,
    })
}

pub fn open(profile: &ShellProfile, id: &str, cols: u16, rows: u16) -> Result<OpenPty, String> {
    open_with_prompt(profile, id, cols, rows, None)
}

pub fn open_with_prompt(
    profile: &ShellProfile,
    id: &str,
    cols: u16,
    rows: u16,
    prompt_directory: Option<&std::path::Path>,
) -> Result<OpenPty, String> {
    static CREATION: std::sync::Mutex<()> = std::sync::Mutex::new(());
    crate::console_host::configure_dll_search()?;
    let command = profile.command_with_prompt(id, prompt_directory)?;
    // Serialize host creation and identification; regular input/output is concurrent.
    let creation_lock = CREATION.lock().unwrap_or_else(|p| p.into_inner());
    let creation = crate::console_guard::Creation::begin()?;
    let pair = native_pty_system()
        .openpty(size(cols, rows)?)
        .map_err(|e| e.to_string())?;
    let console = creation.guard()?;
    drop(creation_lock);
    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| e.to_string())?;
    let job = match child
        .as_raw_handle()
        .ok_or_else(|| "셸 프로세스 핸들이 없습니다.".to_string())
        .and_then(|handle| ProcessJob::assign(handle as isize))
    {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            return Err(error);
        }
    };
    drop(pair.slave);
    Ok(OpenPty {
        console,
        master: pair.master,
        reader,
        writer,
        child,
        job,
    })
}
