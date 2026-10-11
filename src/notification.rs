//! WinRT toasts and an unpackaged desktop COM activation server.
use crate::agent::Notice;
use std::{
    ffi::c_void,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{channel, sync_channel, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use windows::{
    core::{
        implement, ComObject, IUnknown, Interface, Ref, Result as WinResult, BOOL, GUID, HSTRING,
        PCWSTR,
    },
    Data::Xml::Dom::XmlDocument,
    Win32::{
        Foundation::{CLASS_E_NOAGGREGATION, E_POINTER, RPC_E_CHANGED_MODE},
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoRegisterClassObject, CoRevokeClassObject,
            CoUninitialize, IClassFactory, IClassFactory_Impl, IPersistFile,
            StructuredStorage::{InitPropVariantFromCLSID, PROPVARIANT},
            CLSCTX_INPROC_SERVER, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
            COINIT_MULTITHREADED, REGCLS_MULTIPLEUSE,
        },
        UI::{
            Notifications::{
                INotificationActivationCallback, INotificationActivationCallback_Impl,
                NOTIFICATION_USER_INPUT_DATA,
            },
            Shell::{
                IShellLinkW, PropertiesSystem::IPropertyStore, SHStrDupW,
                SetCurrentProcessExplicitAppUserModelID, ShellLink,
            },
        },
    },
    UI::Notifications::{ToastNotification, ToastNotificationManager},
};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

pub const APP_ID: &str = "MiniTerm.Hud";
pub const ACTIVATOR: GUID = GUID::from_u128(0xa1b499dc_c27d_4eac_8ff2_4fb9314ed8ae);
const CLSID_TEXT: &str = "{A1B499DC-C27D-4EAC-8FF2-4FB9314ED8AE}";
type Activation = Arc<dyn Fn(String) + Send + Sync>;
#[implement(INotificationActivationCallback)]
struct Activator {
    callback: Activation,
}
impl INotificationActivationCallback_Impl for Activator_Impl {
    fn Activate(
        &self,
        app: &PCWSTR,
        args: &PCWSTR,
        _data: *const NOTIFICATION_USER_INPUT_DATA,
        _count: u32,
    ) -> WinResult<()> {
        let app = unsafe { app.to_string()? };
        let args = unsafe { args.to_string()? };
        if app == APP_ID {
            if let Some(id) = args
                .strip_prefix("session=")
                .filter(|id| uuid::Uuid::parse_str(id).is_ok())
            {
                (self.callback)(id.into());
            }
        }
        Ok(())
    }
}
#[implement(IClassFactory)]
struct Factory {
    callback: Activation,
}
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> WinResult<()> {
        if iid.is_null() || out.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe {
            *out = std::ptr::null_mut();
        }
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let object: INotificationActivationCallback = ComObject::new(Activator {
            callback: self.callback.clone(),
        })
        .into_interface();
        unsafe { object.query(iid, out).ok() }
    }
    fn LockServer(&self, _lock: BOOL) -> WinResult<()> {
        Ok(())
    }
}
struct Apartment(bool);
impl Apartment {
    fn new() -> WinResult<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        Ok(Self(true))
    }
    fn for_shell() -> WinResult<Self> {
        let status = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if status == RPC_E_CHANGED_MODE {
            return Ok(Self(false));
        }
        status.ok()?;
        Ok(Self(true))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            if self.0 {
                CoUninitialize();
            }
        }
    }
}
struct Registration(u32);
impl Registration {
    fn new(callback: Activation) -> WinResult<Self> {
        let factory: IClassFactory = ComObject::new(Factory { callback }).into_interface();
        Ok(Self(unsafe {
            CoRegisterClassObject(
                &ACTIVATOR,
                &factory,
                CLSCTX_LOCAL_SERVER,
                REGCLS_MULTIPLEUSE,
            )?
        }))
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        unsafe {
            let _ = CoRevokeClassObject(self.0);
        }
    }
}

pub struct ToastWorker {
    tx: SyncSender<(Notice, bool)>,
    pub warning: Arc<Mutex<Option<String>>>,
    pub published: Arc<AtomicU64>,
}
impl ToastWorker {
    pub fn new(callback: impl Fn(String) + Send + Sync + 'static) -> Self {
        let (tx, rx) = sync_channel::<(Notice, bool)>(16);
        let warning = Arc::new(Mutex::new(None));
        let thread_warning = warning.clone();
        let published = Arc::new(AtomicU64::new(0));
        let thread_published = published.clone();
        thread::spawn(move || {
            let setup = Apartment::new().and_then(|apartment| {
                Registration::new(Arc::new(callback)).map(|registration| (apartment, registration))
            });
            match setup {
                Ok((_apartment, _registration)) => {
                    for (notice, sound) in rx {
                        if let Err(error) = show(&notice, sound) {
                            *thread_warning.lock().unwrap_or_else(|p| p.into_inner()) =
                                Some(format!("Windows 알림: {error}"));
                        } else {
                            thread_published.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(error) => {
                    *thread_warning.lock().unwrap_or_else(|p| p.into_inner()) =
                        Some(format!("알림 활성화 등록: {error}"));
                }
            }
        });
        Self {
            tx,
            warning,
            published,
        }
    }
    pub fn send(&self, notice: Notice, sound: bool) {
        let _ = self.tx.try_send((notice, sound));
    }
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn show(notice: &Notice, sound: bool) -> WinResult<()> {
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(format!("<toast launch=\"session={}\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>클릭하여 해당 터미널로 돌아가기</text></binding></visual><audio silent=\"{}\"/></toast>", escape(&notice.session_id), escape(&notice.title), !sound)))?;
    let toast = ToastNotification::CreateToastNotification(&document)?;
    toast.SetTag(&HSTRING::from(
        notice.id.chars().take(16).collect::<String>(),
    ))?;
    toast.SetGroup(&HSTRING::from("agent"))?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?.Show(&toast)
}
pub fn set_process_identity() {
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID));
    }
}
fn shortcut() -> Result<PathBuf, String> {
    std::env::var_os("APPDATA")
        .map(|path| PathBuf::from(path).join("Microsoft/Windows/Start Menu/Programs/MiniTerm.lnk"))
        .ok_or("APPDATA가 없습니다.".into())
}
pub fn is_registered() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(format!("Software\\Classes\\AppUserModelId\\{APP_ID}"))
        .is_ok()
}
pub fn diagnostics() -> Result<serde_json::Value, String> {
    let _apartment = Apartment::new().map_err(|e| e.to_string())?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))
        .map_err(|e| e.to_string())?;
    let history = ToastNotificationManager::History()
        .and_then(|history| history.GetHistoryWithId(&HSTRING::from(APP_ID)))
        .map_err(|e| e.to_string())?;
    Ok(
        serde_json::json!({ "registered": is_registered(), "setting": notifier.Setting().map_err(|e| e.to_string())?.0, "historyCount": history.Size().map_err(|e| e.to_string())? }),
    )
}
/// Called by the per-user installer, or explicitly from integration setup.
pub fn register(helper: &Path) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(format!(
            "Software\\Classes\\CLSID\\{CLSID_TEXT}\\LocalServer32"
        ))
        .map_err(|e| e.to_string())?;
    key.set_value("", &format!("\"{}\" --toast-server", helper.display()))
        .map_err(|e| e.to_string())?;
    let (key, _) = hkcu
        .create_subkey(format!("Software\\Classes\\AppUserModelId\\{APP_ID}"))
        .map_err(|e| e.to_string())?;
    key.set_value("DisplayName", &"MiniTerm")
        .map_err(|e| e.to_string())?;
    key.set_value("CustomActivator", &CLSID_TEXT)
        .map_err(|e| e.to_string())?;
    key.set_value("MiniTermPath", &helper.display().to_string())
        .map_err(|e| e.to_string())?;
    create_shortcut(helper).map_err(|e| e.to_string())
}
fn create_shortcut(helper: &Path) -> WinResult<()> {
    let _apartment = Apartment::for_shell()?;
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(helper.display().to_string()))?;
        link.SetDescription(&HSTRING::from("MiniTerm · Bash HUD"))?;
        let properties: IPropertyStore = link.cast()?;
        let fmtid = GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3);
        let mut app_id = PROPVARIANT::default();
        let inner = &mut *app_id.Anonymous.Anonymous;
        inner.vt = windows::Win32::System::Variant::VT_LPWSTR;
        inner.Anonymous.pwszVal = SHStrDupW(&HSTRING::from(APP_ID))?;
        properties.SetValue(
            &windows::Win32::Foundation::PROPERTYKEY { fmtid, pid: 5 },
            &app_id,
        )?;
        properties.SetValue(
            &windows::Win32::Foundation::PROPERTYKEY { fmtid, pid: 26 },
            &InitPropVariantFromCLSID(&ACTIVATOR)?,
        )?;
        properties.Commit()?;
        let persist: IPersistFile = link.cast()?;
        let path = shortcut()
            .map_err(|e| windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e))?;
        persist.Save(&HSTRING::from(path.display().to_string()), true)
    }
}
pub fn unregister(helper: &Path) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = format!("Software\\Classes\\AppUserModelId\\{APP_ID}");
    let owned = hkcu
        .open_subkey(&key)
        .and_then(|key| key.get_value::<String, _>("MiniTermPath"))
        .is_ok_and(|path| Path::new(&path) == helper);
    if !owned {
        return Ok(());
    }
    if let Ok(_apartment) = Apartment::new() {
        if let Ok(history) = ToastNotificationManager::History() {
            let _ = history.ClearWithId(&HSTRING::from(APP_ID));
        }
    }
    hkcu.delete_subkey_all(key).map_err(|e| e.to_string())?;
    hkcu.delete_subkey_all(format!("Software\\Classes\\CLSID\\{CLSID_TEXT}"))
        .map_err(|e| e.to_string())?;
    let path = shortcut()?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
/// Windows starts this local COM server when a toast outlives the app process.
pub fn activation_server() -> Result<(), String> {
    let _apartment = Apartment::new().map_err(|e| e.to_string())?;
    let (tx, rx) = channel();
    let _registration = Registration::new(Arc::new(move |session_id| {
        let _ = tx.send(session_id);
    }))
    .map_err(|e| e.to_string())?;
    if let Ok(session_id) = rx.recv_timeout(Duration::from_secs(30)) {
        if crate::ipc::send(&crate::ipc::Request::Activate { session_id }).is_err() {
            let mut command =
                std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
            use std::os::windows::process::CommandExt;
            command
                .arg("--stale-notice")
                .creation_flags(0x08000000)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
