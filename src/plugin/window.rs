use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, ShowWindow, CW_USEDEFAULT,
    SW_SHOWDEFAULT, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

use std::ffi::c_void;
use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

fn to_wide(string: &str) -> Vec<u16> {
    OsStr::new(string).encode_wide().chain(Some(0)).collect()
}

pub fn destroy_handler(handler: *mut c_void) {
    unsafe { DestroyWindow(HWND(handler)).unwrap() };
}

pub fn create_handler(_resizable: bool, width: u32, height: u32) -> *mut c_void {
    unsafe {
        let class_name = to_wide("SawaviPluginClass");
        let hinstance = GetModuleHandleW(None).unwrap();

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: HINSTANCE::from(hinstance),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        RegisterClassW(&wnd_class);

        let hwnd = CreateWindowExW(
            Default::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide("Sawavi Plugin").as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            width as i32,
            height as i32,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .unwrap();

        let _ = ShowWindow(hwnd, SW_SHOWDEFAULT);
        let _ = UpdateWindow(hwnd);

        hwnd.0
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
