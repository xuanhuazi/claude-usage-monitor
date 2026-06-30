//! System tray: a programmatically-drawn pie gauge that fills with 5h usage
//! (color shifts coral -> amber -> red), plus a tooltip and context menu.

use crate::types::Status;
use tauri::image::Image;
use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

fn status_color(pct: f64) -> (u8, u8, u8) {
    if pct >= 0.90 {
        (0xC1, 0x5F, 0x3C) // red
    } else if pct >= 0.75 {
        (0xD9, 0xA4, 0x41) // amber
    } else {
        (0xD9, 0x77, 0x57) // Claude coral
    }
}

/// Draw a donut gauge filled clockwise from the top to `pct`.
pub fn render_gauge_icon(pct: f64) -> Image<'static> {
    let size: u32 = 32;
    let (fr, fg, fb) = status_color(pct);
    let (tr, tg, tb) = (0x6B, 0x6B, 0x66); // warm-gray track
    let cx = (size as f32 - 1.0) / 2.0;
    let cy = cx;
    let outer = size as f32 / 2.0 - 1.0;
    let inner = outer * 0.56;
    let frac = pct.clamp(0.0, 1.0) as f32;

    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if dist <= outer && dist >= inner {
                let mut ang = dx.atan2(-dy); // 0 at top, clockwise positive
                if ang < 0.0 {
                    ang += std::f32::consts::TAU;
                }
                let norm = ang / std::f32::consts::TAU;
                if (norm as f32) <= frac {
                    data[idx] = fr;
                    data[idx + 1] = fg;
                    data[idx + 2] = fb;
                    data[idx + 3] = 255;
                } else {
                    data[idx] = tr;
                    data[idx + 1] = tg;
                    data[idx + 2] = tb;
                    data[idx + 3] = 90;
                }
            }
        }
    }
    Image::new_owned(data, size, size)
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text("show", "显示主窗口")
        .text("refresh", "立即刷新")
        .separator()
        .text("quit", "退出")
        .build()?;

    let _tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Claude 额度监控")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_window(app),
            "refresh" => {
                let h = app.clone();
                std::thread::spawn(move || {
                    crate::tick(&h);
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn update_tray(app: &AppHandle, status: &Status) {
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_icon(Some(render_gauge_icon(status.window5h.pct)));
        let pct5 = (status.window5h.pct * 100.0).round() as i64;
        let pct7 = (status.window7d.pct * 100.0).round() as i64;
        let tip = format!("{} — 5h {}% · 7d {}%", status.plan_label, pct5, pct7);
        let _ = tray.set_tooltip(Some(tip));
    }
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
