//! Tray icon and menu (#27): show or hide the buddies, install, repair or
//! uninstall the Claude Code hooks, open the settings window, quit.
//!
//! Hook actions run on their own thread: they show blocking dialogs and
//! touch the disk, and the tray's event handler runs on the main thread.

mod dialog;
pub mod health;
pub mod hooks;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::win::Overlay;
use hooks::HookActions;

const TOGGLE: &str = "toggle";
const INSTALL: &str = "install-hooks";
const UNINSTALL: &str = "uninstall-hooks";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

const TITLE: &str = "Claude Buddy";

struct TrayState {
    toggle: MenuItem<Wry>,
    /// A hook action (and its dialogs) is in progress.
    busy: AtomicBool,
}

/// Adds the tray icon. Call from `setup`, after the overlay is managed.
pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, TOGGLE, "Hide buddies", true, None::<&str>)?;
    let install = MenuItem::with_id(app, INSTALL, "Install / repair hooks…", true, None::<&str>)?;
    let uninstall = MenuItem::with_id(app, UNINSTALL, "Uninstall hooks…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, SETTINGS, "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "Quit Claude Buddy", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &PredefinedMenuItem::separator(app)?,
            &install,
            &uninstall,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &quit,
        ],
    )?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip(TITLE)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TOGGLE => {
                let visible = app.state::<Arc<Overlay>>().visible();
                set_buddies_visible(app, !visible);
            }
            INSTALL => run_hook_action(app, Action::Install),
            UNINSTALL => run_hook_action(app, Action::Uninstall),
            SETTINGS => crate::settings::open(app),
            QUIT => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    app.manage(TrayState {
        toggle,
        busy: AtomicBool::new(false),
    });
    Ok(())
}

/// Shows or hides the overlay and keeps the menu label in step. Hidden
/// buddies are not rendered at all.
pub fn set_buddies_visible(app: &AppHandle, visible: bool) {
    let overlay = app.state::<Arc<Overlay>>();
    if let Err(error) = overlay.set_visible(visible) {
        eprintln!("claude-buddy: could not change overlay visibility: {error}");
        return;
    }
    if let Some(tray) = app.try_state::<TrayState>() {
        let label = if visible {
            "Hide buddies"
        } else {
            "Show buddies"
        };
        let _ = tray.toggle.set_text(label);
    }
}

#[derive(Clone, Copy)]
enum Action {
    Install,
    Uninstall,
}

fn run_hook_action(app: &AppHandle, action: Action) {
    let Some(tray) = app.try_state::<TrayState>() else {
        return;
    };
    // One dialog flow at a time.
    if tray.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("tray-hooks".into())
        .spawn(move || {
            match hook_actions() {
                Ok(actions) => {
                    let confirm = |summary: &str| dialog::confirm(TITLE, summary);
                    let message = match action {
                        Action::Install => actions.install(confirm),
                        Action::Uninstall => actions.uninstall(confirm),
                    };
                    if let Some(message) = message {
                        dialog::inform(TITLE, &message.text, message.warning);
                    }
                }
                Err(text) => dialog::inform(TITLE, &text, true),
            }
            if let Some(tray) = app.try_state::<TrayState>() {
                tray.busy.store(false, Ordering::SeqCst);
            }
        });
    if spawned.is_err() {
        tray.busy.store(false, Ordering::SeqCst);
    }
}

/// Where the settings, the hook binary and the bridge file are.
pub(crate) fn hook_actions() -> Result<HookActions, String> {
    let settings = crate::installer::default_settings_path()
        .ok_or("Could not find the Claude Code configuration folder.")?;
    let exe = std::env::current_exe().map_err(|e| format!("Could not locate Claude Buddy: {e}"))?;
    let hook: PathBuf = exe.with_file_name(if cfg!(windows) {
        "buddy-hook.exe"
    } else {
        "buddy-hook"
    });
    let bridge_file =
        bridge_protocol::bridge_file_path().ok_or("Could not locate the bridge file folder.")?;
    Ok(HookActions {
        settings,
        hook,
        bridge_file,
    })
}
