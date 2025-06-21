use anyhow::Result;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW,
    RegisterClassW, SetWindowLongPtrW, SetWindowPos, ShowWindow, CREATESTRUCTW, CW_USEDEFAULT,
    GWLP_USERDATA, SWP_NOMOVE, SWP_NOZORDER, SW_SHOWDEFAULT, WM_CREATE, WM_DESTROY, WM_SIZE,
    WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

use std::ffi::c_void;
use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use super::Plugin;

fn to_wide(string: &str) -> Vec<u16> {
    OsStr::new(string).encode_wide().chain(Some(0)).collect()
}

pub fn destroy_handler(handler: *mut c_void) {
    unsafe { DestroyWindow(HWND(handler)).unwrap() };
}

pub fn create_handler(
    _resizable: bool,
    width: u32,
    height: u32,
    host_data: *mut c_void,
    hwnd: isize,
) -> *mut c_void {
    unsafe {
        let class_name = to_wide("SingLikeCodingPluginClass");
        let hinstance = GetModuleHandleW(None).unwrap();

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: HINSTANCE::from(hinstance),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        RegisterClassW(&wnd_class);

        let (adjusted_width, adjusted_height) = adjust_size(width, height);

        let hwnd = CreateWindowExW(
            Default::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide("Sing Like Coding Plugin").as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            adjusted_width as i32,
            adjusted_height as i32,
            Some(HWND(hwnd as *mut c_void)),
            None,
            Some(hinstance.into()),
            Some(host_data),
        )
        .unwrap();

        let _ = ShowWindow(hwnd, SW_SHOWDEFAULT);
        let _ = UpdateWindow(hwnd);

        hwnd.0
    }
}

pub fn resize(hwnd: *mut c_void, width: u32, height: u32) -> Result<()> {
    let (adjusted_width, adjusted_height) = adjust_size(width, height);
    unsafe {
        SetWindowPos(
            HWND(hwnd),
            None,
            0,
            0,
            adjusted_width,
            adjusted_height,
            SWP_NOZORDER | SWP_NOMOVE,
        )
        .unwrap();
    }
    Ok(())
}

fn adjust_size(width: u32, height: u32) -> (i32, i32) {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: width as i32,
        bottom: height as i32,
    };

    // ウィンドウスタイルに合わせて調整（WS_OVERLAPPEDWINDOW は枠あり）
    unsafe {
        AdjustWindowRectEx(
            &mut rect,
            WS_OVERLAPPEDWINDOW,
            false, // メニューなし
            Default::default(),
        )
        .unwrap()
    };

    let adjusted_width = rect.right - rect.left;
    let adjusted_height = rect.bottom - rect.top;

    (adjusted_width, adjusted_height)
}

fn LOWORD(lparam: LPARAM) -> u32 {
    (lparam.0 as usize & 0xFFFF) as u32
}

fn HIWORD(lparam: LPARAM) -> u32 {
    ((lparam.0 as usize >> 16) & 0xFFFF) as u32
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
            let ptr = create_struct.lpCreateParams as *mut c_void;
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize) };
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            let plugin = unsafe { &mut *(ptr as *mut Plugin) };
            plugin.gui_close().unwrap();
            LRESULT(0)
        }
        WM_SIZE => {
            let width = LOWORD(lparam);
            let height = HIWORD(lparam);
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
            let plugin = unsafe { &mut *(ptr as *mut Plugin) };
            plugin.gui_size(width, height).unwrap();
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
