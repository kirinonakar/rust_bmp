use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Write, BufWriter};
use byteorder::{WriteBytesExt, LittleEndian};
use image::{DynamicImage, GenericImageView};
use slint::ComponentHandle;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, SetWindowLongPtrW, GWLP_WNDPROC, WM_DROPFILES, WNDPROC,
};

slint::include_modules!();

fn save_32bit_bmp(img: &DynamicImage, output_path: &Path) -> anyhow::Result<()> {
    let (width, height) = img.dimensions();
    let file = File::create(output_path)?;
    let mut writer = BufWriter::new(file);

    // Image data in BGRA8 format
    let rgba = img.to_rgba8();
    
    // BMP File Header (14 bytes)
    writer.write_all(b"BM")?;
    let header_size = 14 + 40; // FileHeader + BITMAPINFOHEADER
    let pixel_data_offset = header_size as u32;
    let file_size = pixel_data_offset + (width * height * 4);
    
    writer.write_u32::<LittleEndian>(file_size)?;
    writer.write_u16::<LittleEndian>(0)?; // Reserved 1
    writer.write_u16::<LittleEndian>(0)?; // Reserved 2
    writer.write_u32::<LittleEndian>(pixel_data_offset)?;

    // BITMAPINFOHEADER (40 bytes)
    writer.write_u32::<LittleEndian>(40)?; // biSize
    writer.write_i32::<LittleEndian>(width as i32)?; // biWidth
    writer.write_i32::<LittleEndian>(height as i32)?; // biHeight (positive for bottom-up)
    writer.write_u16::<LittleEndian>(1)?; // biPlanes
    writer.write_u16::<LittleEndian>(32)?; // biBitCount
    writer.write_u32::<LittleEndian>(0)?; // biCompression (BI_RGB)
    writer.write_u32::<LittleEndian>(0)?; // biSizeImage
    writer.write_i32::<LittleEndian>(0)?; // biXPelsPerMeter
    writer.write_i32::<LittleEndian>(0)?; // biYPelsPerMeter
    writer.write_u32::<LittleEndian>(0)?; // biClrUsed
    writer.write_u32::<LittleEndian>(0)?; // biClrImportant

    // Pixel Data (Bottom-Up)
    for y in (0..height).rev() {
        for x in 0..width {
            let pixel = rgba.get_pixel(x, y);
            writer.write_u8(pixel[2])?; // B
            writer.write_u8(pixel[1])?; // G
            writer.write_u8(pixel[0])?; // R
            writer.write_u8(pixel[3])?; // A
        }
    }

    writer.flush()?;
    Ok(())
}

fn process_file(path_str: &str, handle: AppWindow) {
    let path = PathBuf::from(path_str);
    if !path.exists() || path.extension().and_then(|s| s.to_str()) != Some("png") {
        handle.set_status_text("Error: Please drop a .png file".into());
        return;
    }

    handle.set_status_text(format!("Processing: {}", path.file_name().unwrap().to_string_lossy()).into());

    let img = match image::open(&path) {
        Ok(i) => i,
        Err(e) => {
            handle.set_status_text(format!("Error opening image: {}", e).into());
            return;
        }
    };

    let output_path = path.with_extension("bmp");
    
    match save_32bit_bmp(&img, &output_path) {
        Ok(_) => {
            handle.set_status_text("Success!".into());
            handle.set_sub_status(format!("Saved to: {}", output_path.file_name().unwrap().to_string_lossy()).into());
        }
        Err(e) => {
            handle.set_status_text(format!("Error saving BMP: {}", e).into());
        }
    }
}

use std::sync::OnceLock;
static APP_WINDOW_HANDLE: OnceLock<slint::Weak<AppWindow>> = OnceLock::new();
static mut ORIGINAL_WNDPROC: WNDPROC = None;

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_DROPFILES {
        let hdrop = wparam as HDROP;
        let mut path_buf = [0u16; 512];
        unsafe {
            let count = DragQueryFileW(hdrop, 0, path_buf.as_mut_ptr(), 512);
            if count > 0 {
                let path = String::from_utf16_lossy(&path_buf[..count as usize]);
                if let Some(weak) = APP_WINDOW_HANDLE.get() {
                    let weak_clone = weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = weak_clone.upgrade() {
                            process_file(&path, ui);
                        }
                    });
                }
            }
            DragFinish(hdrop);
        }
        return 0;
    }
    unsafe {
        CallWindowProcW(ORIGINAL_WNDPROC, hwnd, msg, wparam, lparam)
    }
}

fn main() -> anyhow::Result<()> {
    let ui = AppWindow::new()?;
    let ui_handle = ui.as_weak();

    // Set up global handle for WndProc
    let _ = APP_WINDOW_HANDLE.set(ui_handle.clone());

    ui.on_load_clicked({
        let ui_handle = ui_handle.clone();
        move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG Image", &["png"])
                .pick_file() 
            {
                if let Some(ui) = ui_handle.upgrade() {
                    process_file(&path.to_string_lossy(), ui);
                }
            }
        }
    });

    ui.on_setup_hook({
        let ui_handle = ui_handle.clone();
        move || {
            #[cfg(target_os = "windows")]
            {
                if let Some(ui) = ui_handle.upgrade() {
                    use raw_window_handle::HasWindowHandle;
                    let window_handle = ui.window().window_handle();
                    
                    if let Ok(handle) = window_handle.window_handle() {
                        let raw_handle = handle.as_raw();
                        if let raw_window_handle::RawWindowHandle::Win32(h) = raw_handle {
                            let hwnd = h.hwnd.get() as HWND;
                            unsafe {
                                DragAcceptFiles(hwnd, 1);
                                ORIGINAL_WNDPROC = core::mem::transmute(SetWindowLongPtrW(
                                    hwnd,
                                    GWLP_WNDPROC,
                                    wnd_proc as *const () as isize,
                                ));
                            }
                        }
                    }
                }
            }
        }
    });

    ui.run()?;
    Ok(())
}
