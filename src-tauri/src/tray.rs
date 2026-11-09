// Windows-first tray: icon, menu, and main window toggle.

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

const MAIN_WINDOW: &str = "main";

pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Kaminari", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &separator, &quit])?;

    let Some(icon) = app.default_window_icon() else {
        tracing::warn!("tray: default window icon is missing");
        return Ok(());
    };

    let tray = TrayIconBuilder::with_id("main")
        .icon(icon.clone())
        .tooltip("Kaminari")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
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
                toggle_main(tray.app_handle());
            }
        })
        .build(app)?;

    app.manage(tray);
    Ok(())
}

fn show_main<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    if let Err(err) = window.show() {
        tracing::warn!(error = %err, "tray: could not show main window");
        return;
    }
    if let Err(err) = window.set_focus() {
        tracing::warn!(error = %err, "tray: could not focus main window");
    }
}

fn toggle_main<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    match window.is_visible() {
        Ok(true) => {
            if let Err(err) = window.hide() {
                tracing::warn!(error = %err, "tray: could not hide main window");
            }
        }
        Ok(false) => show_main(app),
        Err(err) => tracing::warn!(error = %err, "tray: could not read window visibility"),
    }
}
