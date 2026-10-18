//! Application identity and compositor hints for the native game window.

#[cfg(target_os = "linux")]
pub fn configure(window: &minifb::Window) -> Result<(), Box<dyn std::error::Error>> {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
    use std::{ffi::CString, os::raw::c_ulong};
    use x11_dl::xlib;

    let (RawDisplayHandle::Xlib(display), RawWindowHandle::Xlib(handle)) = (
        window.display_handle()?.as_raw(),
        window.window_handle()?.as_raw(),
    ) else {
        // X11 compositor hints do not apply to Wayland windows.
        return Ok(());
    };
    let Some(display) = display.display else {
        return Ok(());
    };
    let xlib = xlib::Xlib::open()?;
    let name = CString::new("micro-mages-rust")?;
    let class = CString::new("MicroMagesRust")?;
    let mut hint = xlib::XClassHint {
        res_name: name.as_ptr().cast_mut(),
        res_class: class.as_ptr().cast_mut(),
    };
    let opacity: c_ulong = 0xffff_ffff;
    // SAFETY: minifb owns this live Xlib display/window for the duration of this
    // call. Xlib copies the class strings and the one 32-bit property element.
    unsafe {
        let display = display.as_ptr().cast();
        (xlib.XSetClassHint)(display, handle.window, &mut hint);
        let atom = (xlib.XInternAtom)(display, c"_NET_WM_WINDOW_OPACITY".as_ptr(), xlib::False);
        (xlib.XChangeProperty)(
            display,
            handle.window,
            atom,
            xlib::XA_CARDINAL,
            32,
            xlib::PropModeReplace,
            (&opacity as *const c_ulong).cast(),
            1,
        );
        (xlib.XFlush)(display);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn configure(_window: &minifb::Window) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
