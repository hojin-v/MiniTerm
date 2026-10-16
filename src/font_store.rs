//! Read-only, process-lifetime mappings for the seven fixed Windows font faces.
use std::{
    fs::OpenOptions,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
};
use windows_sys::Win32::{
    Foundation::CloseHandle,
    Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ},
    System::Memory::{CreateFileMappingW, MapViewOfFile, FILE_MAP_READ, PAGE_READONLY},
};

// Keep this private to app's fixed LazyLock font catalog. User-selected fonts
// remain owned, replaceable buffers and never enter this process-lifetime store.
pub(crate) fn system_face(path: &Path) -> Option<&'static [u8]> {
    // Reject concurrent in-place writes while the section is in use. Atomic OS
    // font replacement is allowed; the old view continues to name the old file.
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(path)
        .ok()?;
    let len: usize = file.metadata().ok()?.len().try_into().ok()?;
    if len == 0 || len > 64 * 1024 * 1024 {
        return None;
    }
    unsafe {
        let mapping = CreateFileMappingW(
            file.as_raw_handle() as _,
            std::ptr::null(),
            PAGE_READONLY,
            0,
            0,
            std::ptr::null(),
        );
        if mapping.is_null() {
            return None;
        }
        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, len);
        CloseHandle(mapping);
        if view.Value.is_null() {
            return None;
        }
        // Keep the seven file handles too, so the deny-write sharing contract
        // protects FontRef's immutable bytes for the full process lifetime.
        std::mem::forget(file);
        // Windows keeps the section alive after its mapping handle closes.
        // Intentionally retain exactly these fixed views
        // until process exit, matching FontData::from_static's lifetime. Pages
        // are file-backed and read on demand instead of copied into private RAM.
        Some(std::slice::from_raw_parts(view.Value.cast(), len))
    }
}
