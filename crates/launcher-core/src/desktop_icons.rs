//! Reads and sets where icons sit on the Windows desktop.
//!
//! The desktop is an Explorer folder view. We reach it the documented way
//! (`IShellWindows::FindWindowSW(SWC_DESKTOP)` → `IShellBrowser` →
//! `IFolderView`) and use `GetItemPosition` / `SelectAndPositionItems`.
//! Items are matched by their file name (e.g. `Cyberpunk 2077.lnk`).
//!
//! Positions are only a convenience: every function here fails soft and
//! returns "nothing done" when Explorer isn't running, auto-arrange is on,
//! or the item isn't visible yet. On other OSes they are no-ops.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// An icon position in desktop view coordinates (pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconPosition {
    pub x: i32,
    pub y: i32,
}

/// Lowercased file name, the key used to match desktop items.
pub fn key(file_name: &str) -> String {
    file_name.to_lowercase()
}

/// Positions of all items on the desktop, keyed by [`key`] of their file name.
pub fn read_positions() -> HashMap<String, IconPosition> {
    run_on_sta(imp::read_positions).unwrap_or_default()
}

/// Moves the named items (file names) to the given positions. Returns the
/// file names that were placed; items not (yet) on the desktop are skipped.
pub fn apply_positions(items: &[(String, IconPosition)]) -> Vec<String> {
    let items = items.to_vec();
    run_on_sta(move || imp::apply_positions(&items)).unwrap_or_default()
}

/// Like [`apply_positions`], but retries until every item was placed or the
/// timeout ran out. Explorer notices new files with a short delay, so this
/// is what to call right after files were put back on the desktop.
pub fn apply_positions_when_visible(items: Vec<(String, IconPosition)>, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    let mut left = items;
    while !left.is_empty() {
        let placed = apply_positions(&left);
        left.retain(|(name, _)| !placed.iter().any(|p| key(p) == key(name)));
        if left.is_empty() || Instant::now() >= deadline || !cfg!(windows) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Runs `f` on a fresh thread with its own single-threaded COM apartment, so
/// callers don't need to care which thread (or apartment) they are on.
fn run_on_sta<T: Send + 'static>(f: impl FnOnce() -> Option<T> + Send + 'static) -> Option<T> {
    std::thread::spawn(move || {
        let _com = imp::ComGuard::init();
        f()
    })
    .join()
    .ok()
    .flatten()
}

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::mem::ManuallyDrop;

    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{
        CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree, CoUninitialize, IServiceProvider,
    };
    use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VT_I4};
    use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
    use windows::Win32::UI::Shell::{
        CSIDL_DESKTOP, IEnumIDList, IFolderView, IShellBrowser, IShellFolder, IShellWindows,
        SHGDN_FORPARSING, SHGDN_INFOLDER, SHGDNF, SID_STopLevelBrowser, SVGIO_ALLVIEW,
        SVSI_POSITIONITEM, SWC_DESKTOP, SWFO_NEEDDISPATCH, ShellWindows, StrRetToStrW,
    };
    use windows::core::{Interface, PWSTR};

    use super::{IconPosition, key};

    pub struct ComGuard(bool);

    impl ComGuard {
        pub fn init() -> Self {
            // SAFETY: plain COM initialisation of this (fresh) thread.
            Self(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: paired with the successful CoInitializeEx above.
                unsafe { CoUninitialize() };
            }
        }
    }

    /// Frees a PIDL returned by the shell when dropped.
    struct Pidl(*mut ITEMIDLIST);

    impl Drop for Pidl {
        fn drop(&mut self) {
            // SAFETY: the pointer came from IEnumIDList::Next (CoTaskMemAlloc).
            unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
        }
    }

    fn desktop_view() -> windows::core::Result<IFolderView> {
        // SAFETY: standard COM calls; all out-values are owned wrappers.
        unsafe {
            let windows: IShellWindows =
                CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER)?;
            let loc = VARIANT {
                Anonymous: VARIANT_0 {
                    Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                        vt: VT_I4,
                        Anonymous: windows::Win32::System::Variant::VARIANT_0_0_0 {
                            lVal: CSIDL_DESKTOP as i32,
                        },
                        ..Default::default()
                    }),
                },
            };
            let empty = VARIANT::default();
            let mut hwnd = 0i32;
            let dispatch =
                windows.FindWindowSW(&loc, &empty, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH)?;
            let provider: IServiceProvider = dispatch.cast()?;
            let browser: IShellBrowser = provider.QueryService(&SID_STopLevelBrowser)?;
            browser.QueryActiveShellView()?.cast()
        }
    }

    /// All items in the view with their parsing name (file name).
    fn items(view: &IFolderView) -> windows::core::Result<Vec<(String, Pidl)>> {
        // SAFETY: COM calls on a live view; PIDLs are freed by `Pidl`.
        unsafe {
            let folder: IShellFolder = view.GetFolder()?;
            let list: IEnumIDList = view.Items(SVGIO_ALLVIEW)?;
            let mut out = Vec::new();
            loop {
                let mut buf = [std::ptr::null_mut(); 1];
                let mut fetched = 0u32;
                if list.Next(&mut buf, Some(&mut fetched)).is_err() || fetched == 0 {
                    break;
                }
                let pidl = Pidl(buf[0]);
                let mut name = STRRET::default();
                if folder
                    .GetDisplayNameOf(
                        pidl.0,
                        SHGDNF(SHGDN_INFOLDER.0 | SHGDN_FORPARSING.0),
                        &mut name,
                    )
                    .is_err()
                {
                    continue;
                }
                let mut text = PWSTR::null();
                if StrRetToStrW(&mut name, Some(pidl.0), &mut text).is_err() {
                    continue;
                }
                let s = text.to_string().unwrap_or_default();
                CoTaskMemFree(Some(text.0 as *const _));
                out.push((s, pidl));
            }
            Ok(out)
        }
    }

    pub fn read_positions() -> Option<HashMap<String, IconPosition>> {
        let view = desktop_view().ok()?;
        let mut map = HashMap::new();
        for (name, pidl) in items(&view).ok()? {
            // SAFETY: pidl belongs to this view.
            if let Ok(p) = unsafe { view.GetItemPosition(pidl.0) } {
                map.insert(key(&name), IconPosition { x: p.x, y: p.y });
            }
        }
        Some(map)
    }

    pub fn apply_positions(wanted: &[(String, IconPosition)]) -> Option<Vec<String>> {
        let view = desktop_view().ok()?;
        let wanted: HashMap<String, IconPosition> =
            wanted.iter().map(|(n, p)| (key(n), *p)).collect();
        let mut placed = Vec::new();
        for (name, pidl) in items(&view).ok()? {
            let Some(pos) = wanted.get(&key(&name)) else {
                continue;
            };
            let pt = POINT { x: pos.x, y: pos.y };
            let apidl = [pidl.0 as *const ITEMIDLIST];
            // SAFETY: one PIDL of this view and one point, as the API expects.
            let ok = unsafe {
                view.SelectAndPositionItems(
                    1,
                    apidl.as_ptr(),
                    Some(&pt),
                    SVSI_POSITIONITEM.0 as u32,
                )
            };
            if ok.is_ok() {
                placed.push(name);
            }
        }
        Some(placed)
    }
}

#[cfg(not(windows))]
mod imp {
    use std::collections::HashMap;

    use super::IconPosition;

    pub struct ComGuard;

    impl ComGuard {
        pub fn init() -> Self {
            Self
        }
    }

    pub fn read_positions() -> Option<HashMap<String, IconPosition>> {
        None
    }

    pub fn apply_positions(_: &[(String, IconPosition)]) -> Option<Vec<String>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_ignore_case() {
        assert_eq!(key("Cyberpunk 2077.LNK"), key("cyberpunk 2077.lnk"));
    }

    #[test]
    fn soft_fails_without_a_desktop() {
        // In CI/Linux there is no Explorer desktop: nothing happens, no panic.
        if cfg!(windows) {
            return;
        }
        assert!(read_positions().is_empty());
        apply_positions_when_visible(
            vec![("x.lnk".into(), IconPosition { x: 1, y: 2 })],
            Duration::from_millis(10),
        );
    }

    #[test]
    fn position_serializes_compactly() {
        let p = IconPosition { x: 10, y: -4 };
        assert_eq!(serde_json::to_string(&p).unwrap(), r#"{"x":10,"y":-4}"#);
    }
}
