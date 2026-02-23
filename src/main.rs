use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Write, BufWriter};
use byteorder::{WriteBytesExt, LittleEndian};
use image::{DynamicImage, GenericImageView};
use slint::ComponentHandle;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, ChangeWindowMessageFilterEx, SetWindowLongPtrW, GWLP_WNDPROC, MSGFLT_ALLOW,
    WM_DROPFILES, WNDPROC,
};
use std::sync::OnceLock;

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

static APP_WINDOW_HANDLE: OnceLock<slint::Weak<AppWindow>> = OnceLock::new();
static mut ORIGINAL_WNDPROC: WNDPROC = None;

// wnd_proc에 디버그 로그 추가
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DROPFILES => {
            println!("파일 드롭 감지됨! (WM_DROPFILES)"); // 디버그 로그
            let hdrop = wparam as HDROP;
            let mut path_buf = [0u16; 1024]; // 버퍼 크기 넉넉하게
            unsafe {
                // 0을 사용하여 첫 번째 파일만 가져옴
                let len = DragQueryFileW(hdrop, 0, path_buf.as_mut_ptr(), 1024);
                if len > 0 {
                    let path = String::from_utf16_lossy(&path_buf[..len as usize]);
                    println!("드롭된 파일 경로: {}", path); // 경로 확인

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
            return 0; // 드롭 처리 완료 시 0 반환
        }
        _ => {}
    }
    
    // 원래 윈도우 프로시저 호출 (체이닝)
    unsafe {
        if let Some(orig) = ORIGINAL_WNDPROC {
            CallWindowProcW(Some(orig), hwnd, msg, wparam, lparam)
        } else {
            windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

fn main() -> anyhow::Result<()> {
    let ui = AppWindow::new()?;
    let ui_handle = ui.as_weak();

    // 전역 핸들 설정
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

    // 1. 창 띄우기 (HWND 생성)
    ui.show()?;

    // 2. Windows API 후킹 (Win32 핸들 추출 로직 개선)
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        
        let window_handle = ui.window().window_handle();
        // HasWindowHandle trait의 window_handle() 호출
        match window_handle.window_handle() {
            Ok(handle) => {
                // 핸들의 Raw 타입 확인
                match handle.as_raw() {
                    RawWindowHandle::Win32(handle) => {
                        let hwnd = handle.hwnd.get() as HWND;
                        println!("HWND 획득 성공: {:?}", hwnd);

                        unsafe {
                            // 관리자 권한 UIPI 우회 설정
                            ChangeWindowMessageFilterEx(hwnd, WM_DROPFILES, MSGFLT_ALLOW, std::ptr::null_mut());
                            ChangeWindowMessageFilterEx(hwnd, 0x0049, MSGFLT_ALLOW, std::ptr::null_mut()); 
                            
                            // 드래그 앤 드롭 활성화
                            DragAcceptFiles(hwnd, 1);
                            println!("DragAcceptFiles 설정 완료");

                            // WndProc 교체 (Subclassing)
                            let prev_proc = SetWindowLongPtrW(
                                hwnd,
                                GWLP_WNDPROC,
                                wnd_proc as *const () as isize,
                            );
                            
                            if prev_proc == 0 {
                                println!("경고: SetWindowLongPtrW가 0을 반환했습니다");
                            } else {
                                println!("WndProc 후킹 성공. 이전 주소: {:?}", prev_proc);
                                type WndProcFn = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;
                                ORIGINAL_WNDPROC = Some(core::mem::transmute::<isize, WndProcFn>(prev_proc));
                            }
                        }
                    }
                    _ => {
                        println!("오류: Win32 핸들이 아닙니다.");
                    }
                }
            }
            Err(e) => {
                println!("오류: 윈도우 핸들을 가져올 수 없습니다. 상세: {:?}", e);
            }
        }
    }

    println!("이벤트 루프 시작");
    slint::run_event_loop()?;
    Ok(())
}
