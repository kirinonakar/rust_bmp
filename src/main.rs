use slint::ComponentHandle;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Write, BufWriter};
use byteorder::{WriteBytesExt, LittleEndian};
use image::{DynamicImage, GenericImageView};

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

fn main() -> anyhow::Result<()> {
    let ui = AppWindow::new()?;
    let ui_handle = ui.as_weak();

    ui.on_file_dropped(move |path| {
        if let Some(ui) = ui_handle.upgrade() {
            process_file(&path, ui);
        }
    });

    let ui_handle_for_load = ui.as_weak();
    ui.on_load_clicked(move || {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .pick_file() 
        {
            if let Some(ui) = ui_handle_for_load.upgrade() {
                process_file(&path.to_string_lossy(), ui);
            }
        }
    });

    // We need to use Window::on_event or Window::on_file_dropped if available in this Slint version.
    // Actually, Slint's Window has a `window_event` callback or we can handle it at the platform level.
    // However, Slint 1.x has a way to handle dropped files via the Window object directly in some platforms,
    // but the most reliable way for a cross-platform app is to use the `window().on_mouse_input` etc?
    // Wait, Slint has `WindowEvent::DroppedFile` in its event loop.
    
    // For simplicity, I'll use a timer or a dedicated thread if needed, but let's try to 
    // hook into the Window's event loop if possible. 
    // In Slint, you can use `ui.window().on_event(...)` in recent versions.
    
    /*
    let ui_handle_for_events = ui.as_weak();
    ui.window().on_event(move |event| {
        if let slint::platform::WindowEvent::DroppedFile(path) = event {
            if let Some(ui) = ui_handle_for_events.upgrade() {
                process_file(&path.to_string_lossy(), ui);
            }
        }
    });
    */

    ui.run()?;
    Ok(())
}
