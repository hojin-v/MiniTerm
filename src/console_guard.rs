//! Own the new ConPTY host separately from the shell/agent process tree.
use std::{
    collections::HashSet,
    mem::{size_of, zeroed},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Threading::{
            GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
            PROCESS_TERMINATE,
        },
    },
};

pub(crate) const CONSOLE_COMMIT_LIMIT: usize = 256 * 1024 * 1024;

pub struct ConsoleHost {
    job: crate::native::ProcessJob,
    pub pid: u32,
}
impl ConsoleHost {
    pub fn terminate(&self) {
        self.job.terminate();
    }
    #[cfg(test)]
    pub(crate) fn memory_limit(&self) -> usize {
        self.job.process_memory_limit()
    }
    #[cfg(test)]
    pub(crate) fn contains(&self, process: isize) -> bool {
        self.job.contains(process)
    }
}

fn children() -> Result<HashSet<u32>, String> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let snapshot = OwnedHandle::from_raw_handle(snapshot as _);
        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut found = HashSet::new();
        let mut more = Process32FirstW(snapshot.as_raw_handle() as _, &mut entry);
        while more != 0 {
            if entry.th32ParentProcessID == GetCurrentProcessId() {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
                if name.eq_ignore_ascii_case("conhost.exe")
                    || name.eq_ignore_ascii_case("OpenConsole.exe")
                {
                    found.insert(entry.th32ProcessID);
                }
            }
            more = Process32NextW(snapshot.as_raw_handle() as _, &mut entry);
        }
        if GetLastError() != ERROR_NO_MORE_FILES {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(found)
    }
}

pub(crate) struct Creation {
    before: HashSet<u32>,
}
impl Creation {
    pub fn begin() -> Result<Self, String> {
        Ok(Self {
            before: children()?,
        })
    }
    pub fn guard(self) -> Result<ConsoleHost, String> {
        let after = children()?;
        let mut created = after.difference(&self.before).copied();
        let pid = created
            .next()
            .ok_or("새 콘솔 호스트의 자원 보호를 설정하지 못했습니다.")?;
        if created.next().is_some() {
            return Err("동시에 생성된 콘솔 호스트를 구분하지 못했습니다.".into());
        }
        let handle = unsafe {
            OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // Recheck ownership after opening the handle; a stale/reused PID must
        // never let a session take another application's console into its job.
        let result = (|| {
            if !children()?.contains(&pid) {
                return Err("콘솔 호스트 소유권이 변경됐습니다.".into());
            }
            let job = crate::native::ProcessJob::assign_with_memory_limit(
                handle as isize,
                Some(CONSOLE_COMMIT_LIMIT),
            )?;
            Ok(ConsoleHost { job, pid })
        })();
        unsafe {
            CloseHandle(handle);
        }
        result
    }
}
