#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Write, BufWriter};
use byteorder::{WriteBytesExt, LittleEndian};
use image::{DynamicImage, GenericImageView};
use slint::ComponentHandle;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, GetLastError};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, ChangeWindowMessageFilterEx, SetWindowLongPtrW, GWLP_WNDPROC, 
    MSGFLT_ALLOW, WM_DROPFILES, WNDPROC,
};
use windows_sys::Win32::System::Ole::RevokeDragDrop;
use std::sync::OnceLock;

slint::include_modules!();
fn save_32bit_bmp_from_data(width: u32, height: u32, rgba_data: &[u8], output_path: &Path) -> anyhow::Result<()> {
    let file = File::create(output_path)?;
    let mut writer = BufWriter::new(file);

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
    // rgba_data is expected to be [R, G, B, A, ...]
    for y in (0..height).rev() {
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            writer.write_u8(rgba_data[offset + 2])?; // B
            writer.write_u8(rgba_data[offset + 1])?; // G
            writer.write_u8(rgba_data[offset])?;     // R
            writer.write_u8(rgba_data[offset + 3])?; // A
        }
    }

    writer.flush()?;
    Ok(())
}

fn save_32bit_bmp(img: &DynamicImage, output_path: &Path) -> anyhow::Result<()> {
    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();
    save_32bit_bmp_from_data(width, height, &rgba, output_path)
}

fn combine_images(bg_path: &Path, alpha_path: &Path, output_path: &Path) -> anyhow::Result<()> {
    let bg_img = image::open(bg_path)?;
    let alpha_img = image::open(alpha_path)?;

    let (w1, h1) = bg_img.dimensions();
    let (w2, h2) = alpha_img.dimensions();

    if w1 != w2 || h1 != h2 {
        return Err(anyhow::anyhow!("Image dimensions do not match: {}x{} vs {}x{}", w1, h1, w2, h2));
    }

    let bg_rgba = bg_img.to_rgba8();
    let alpha_rgba = alpha_img.to_rgba8();
    
    let mut combined_data = Vec::with_capacity((w1 * h1 * 4) as usize);
    
    for y in 0..h1 {
        for x in 0..w1 {
            let p_bg = bg_rgba.get_pixel(x, y);
            let p_alpha = alpha_rgba.get_pixel(x, y);
            
            combined_data.push(p_bg[0]); // R from background
            combined_data.push(p_bg[1]); // G from background
            combined_data.push(p_bg[2]); // B from background
            combined_data.push(p_alpha[3]); // A from alpha source
        }
    }

    save_32bit_bmp_from_data(w1, h1, &combined_data, output_path)
}

fn load_slint_image(path: &str) -> slint::Image {
    match slint::Image::load_from_path(Path::new(path)) {
        Ok(img) => img,
        Err(_) => slint::Image::default(),
    }
}

fn process_file(path_str: &str, is_right_side: bool, handle: AppWindow) {
    let with_bg = handle.get_with_background();

    if with_bg {
        if path_str.is_empty() {
            // "Combine and Save" button clicked
            let bg_path_str = handle.get_bg_path().to_string();
            let alpha_path_str = handle.get_alpha_path().to_string();
            
            if bg_path_str.is_empty() || alpha_path_str.is_empty() {
                handle.set_status_text("오류: 두 파일이 모두 필요합니다".into());
                return;
            }

            let bg_path = PathBuf::from(&bg_path_str);
            let alpha_path = PathBuf::from(&alpha_path_str);
            let output_path = bg_path.with_extension("combined.bmp");

            handle.set_status_text("이미지 결합 중...".into());
            match combine_images(&bg_path, &alpha_path, &output_path) {
                Ok(_) => {
                    handle.set_status_text("성공!".into());
                    handle.set_sub_status(format!("저장됨: {}", output_path.file_name().unwrap().to_string_lossy()).into());
                }
                Err(e) => {
                    handle.set_status_text(format!("오류: {}", e).into());
                }
            }
            return;
        }

        // File dropped in combined mode
        let path = PathBuf::from(path_str);
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        
        if is_right_side {
            // Right side (Alpha) usually needs to be PNG for transparency
            if ext != "png" {
                handle.set_status_text("오류: 투명 이미지(오른쪽)는 .png 파일이어야 합니다".into());
                return;
            }
            handle.set_alpha_path(path_str.into());
            handle.set_alpha_preview(load_slint_image(path_str));
        } else {
            // Left side (Background) can be PNG, JPG, or BMP
            let allowed = ["png", "jpg", "jpeg", "bmp"];
            if !allowed.contains(&ext.as_str()) {
                handle.set_status_text("오류: 배경(왼쪽)은 .png, .jpg, .bmp 파일만 가능합니다".into());
                return;
            }
            handle.set_bg_path(path_str.into());
            handle.set_bg_preview(load_slint_image(path_str));
        }
        handle.set_status_text("파일이 추가되었습니다. 결과물을 확인하려면 '합쳐서 BMP로 저장'을 누르세요.".into());
    } else {
        // Normal mode - keep as PNG -> BMP
        let path = PathBuf::from(path_str);
        if !path.exists() || path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase() != "png" {
            handle.set_status_text("오류: .png 파일만 가능합니다".into());
            return;
        }

        handle.set_status_text(format!("처리 중: {}", path.file_name().unwrap().to_string_lossy()).into());
        handle.set_single_preview(load_slint_image(path_str));

        let img = match image::open(&path) {
            Ok(i) => i,
            Err(e) => {
                handle.set_status_text(format!("이미지 열기 오류: {}", e).into());
                return;
            }
        };

        let output_path = path.with_extension("bmp");
        
        match save_32bit_bmp(&img, &output_path) {
            Ok(_) => {
                handle.set_status_text("성공!".into());
                handle.set_sub_status(format!("저장됨: {}", output_path.file_name().unwrap().to_string_lossy()).into());
            }
            Err(e) => {
                handle.set_status_text(format!("BMP 저장 오류: {}", e).into());
            }
        }
    }
}

static APP_WINDOW_HANDLE: OnceLock<slint::Weak<AppWindow>> = OnceLock::new();
static mut ORIGINAL_WNDPROC: WNDPROC = None;

// wnd_proc에 디버그 로그 추가
// wnd_proc에 디버그 로그 추가
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DROPFILES => {
            let hdrop = wparam as HDROP;
            let mut path_buf = [0u16; 1024]; 
            let mut pt = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
            
            unsafe {
                windows_sys::Win32::UI::Shell::DragQueryPoint(hdrop, &mut pt);
                
                let len = DragQueryFileW(hdrop, 0, path_buf.as_mut_ptr(), 1024);
                if len > 0 {
                    let path = String::from_utf16_lossy(&path_buf[..len as usize]);
                    
                    if let Some(weak) = APP_WINDOW_HANDLE.get() {
                        let weak_clone = weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = weak_clone.upgrade() {
                                // Determine if dropped on left or right half
                                let window_width = ui.window().size().width as f32;
                                let is_right_side = (pt.x as f32) > (window_width / 2.0);
                                process_file(&path, is_right_side, ui);
                            }
                        });
                    }
                }
                DragFinish(hdrop);
            }
            return 0;
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

    ui.on_file_dropped({
        let ui_handle = ui_handle.clone();
        move |path, is_right| {
            if let Some(ui) = ui_handle.upgrade() {
                process_file(&path, is_right, ui);
            }
        }
    });

    ui.on_combine_clicked({
        let ui_handle = ui_handle.clone();
        move || {
            if let Some(ui) = ui_handle.upgrade() {
                println!("Combine button clicked");
                process_file("", false, ui);
            }
        }
    });

    ui.on_load_clicked({
        let ui_handle = ui_handle.clone();
        move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG Image", &["png"])
                .pick_file() 
            {
                if let Some(ui) = ui_handle.upgrade() {
                    process_file(&path.to_string_lossy(), false, ui);
                }
            }
        }
    });

    // 1. 창 띄우기
    ui.show()?;

    // 2. Windows API 후킹 (이벤트 루프 실행 후 적용되도록 타이머 사용!)
    #[cfg(target_os = "windows")]
    {
        let ui_handle_clone = ui_handle.clone();
        
        // Timer를 사용하여 이벤트 루프가 시작되고 300ms 뒤에 훅을 설치합니다.
        slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
            if let Some(ui) = ui_handle_clone.upgrade() {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                let window_handle = ui.window().window_handle();
                if let Ok(handle) = window_handle.window_handle() {
                    if let RawWindowHandle::Win32(h) = handle.as_raw() {
                        let hwnd = h.hwnd.get() as HWND;
                        println!("Slint HWND 획득 성공 (지연 실행): {:?}", hwnd);

                        unsafe {
                            // 핵심: 이벤트 루프가 덮어씌운 OLE 드래그 앤 드롭을 이 시점에서 빼앗아옵니다.
                            let hr = RevokeDragDrop(hwnd);
                            println!("RevokeDragDrop 실행 (S_OK=0 이면 정상): {}", hr);

                            // 관리자 권한 UIPI 우회 설정
                            ChangeWindowMessageFilterEx(hwnd, WM_DROPFILES, MSGFLT_ALLOW, std::ptr::null_mut());
                            ChangeWindowMessageFilterEx(hwnd, 0x0049, MSGFLT_ALLOW, std::ptr::null_mut()); 
                            ChangeWindowMessageFilterEx(hwnd, 0x004A, MSGFLT_ALLOW, std::ptr::null_mut());
                            
                            // 드래그 앤 드롭 활성화
                            DragAcceptFiles(hwnd, 1);
                            println!("DragAcceptFiles 설정 완료");

                            // WndProc 교체 (Subclassing)
                            let prev_proc = SetWindowLongPtrW(
                                hwnd,
                                GWLP_WNDPROC,
                                wnd_proc as *const () as isize,
                            );
                            
                            if prev_proc != 0 {
                                println!("WndProc 후킹 성공. 이전 주소: 0x{:X}", prev_proc);
                                type WndProcFn = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;
                                ORIGINAL_WNDPROC = Some(core::mem::transmute::<isize, WndProcFn>(prev_proc));
                            } else {
                                println!("경고: SetWindowLongPtrW 실패. 에러 코드: {}", GetLastError());
                            }
                        }
                    }
                }
            }
        });
    }

    println!("이벤트 루프 시작");
    // 3. 이벤트 루프 실행 (이후 타이머가 작동하여 훅이 설치됨)
    slint::run_event_loop()?;
    Ok(())
}
