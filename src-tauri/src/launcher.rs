use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::main_window::MAIN_LABEL;

pub(crate) const LAUNCHER_LABEL: &str = "launcher";
pub(crate) const RELAUNCH_MARKER: &str = "launcher-skip-once";
pub(crate) const RELAUNCH_MARKER_TTL: Duration = Duration::from_secs(30 * 60);

pub(crate) fn is_remote_label(label: &str) -> bool {
    crate::main_window::is_remote_window_label(label)
}

pub(crate) fn remote_count_excluding<'a>(labels: impl Iterator<Item = &'a str>, excluded: &str) -> usize {
    labels.filter(|l| *l != excluded && is_remote_label(l)).count()
}

pub(crate) fn should_show_launcher(has_open_project: bool, internal_relaunch: bool) -> bool {
    !has_open_project && !internal_relaunch
}

pub(crate) fn write_relaunch_marker(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(RELAUNCH_MARKER), b"")
}

pub(crate) fn clear_relaunch_marker(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(RELAUNCH_MARKER));
}

pub(crate) fn consume_relaunch_marker(dir: &Path, now: SystemTime) -> bool {
    let path = dir.join(RELAUNCH_MARKER);
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .map(|mtime| now.duration_since(mtime).map_or(true, |age| age <= RELAUNCH_MARKER_TTL))
        .unwrap_or(false);
    let _ = std::fs::remove_file(&path);
    fresh
}

pub(crate) fn should_offer_return(remaining_remote: usize, main_visible: bool, exiting: bool) -> bool {
    remaining_remote == 0 && !main_visible && !exiting
}

#[derive(Debug, PartialEq)]
pub(crate) enum FocusTarget {
    Main,
    Launcher,
    Remote(String),
}

pub(crate) fn pick_focus_target(main_visible: bool, launcher_open: bool, recent_remotes: &[String]) -> FocusTarget {
    if main_visible {
        FocusTarget::Main
    } else if launcher_open {
        FocusTarget::Launcher
    } else if let Some(newest) = recent_remotes.last() {
        FocusTarget::Remote(newest.clone())
    } else {
        FocusTarget::Main
    }
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HiddenModeCopy {
    pub hide_prompt: String,
    pub hide: String,
    pub keep_open: String,
    pub last_closed_prompt: String,
    pub return_to_local: String,
    pub quit: String,
}

impl Default for HiddenModeCopy {
    fn default() -> Self {
        Self {
            hide_prompt: "Hide the main window while this session is open?".into(),
            hide: "Hide".into(),
            keep_open: "Keep open".into(),
            last_closed_prompt: "The last remote window was closed. Return to the local window?".into(),
            return_to_local: "Return to local".into(),
            quit: "Quit".into(),
        }
    }
}

#[derive(Default)]
pub(crate) struct LauncherState {
    pub exiting: AtomicBool,
    pub focus_order: Mutex<Vec<String>>,
    pub copy: Mutex<HiddenModeCopy>,
}

pub(crate) fn reveal_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

pub(crate) fn hide_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.hide();
    }
}

pub(crate) fn main_visible(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN_LABEL)
        .map_or(true, |win| win.is_visible().unwrap_or(true))
}

/// Called on the main thread right after an SSH window opens. The dialog is non-blocking so the
/// pending `ssh_connect` can return while the user decides.
pub(crate) fn on_ssh_window_opened(app: &AppHandle, caller: &str, ssh_label: &str) {
    if caller == LAUNCHER_LABEL {
        if let Some(win) = app.get_webview_window(LAUNCHER_LABEL) {
            let _ = win.destroy();
        }
        return;
    }
    if caller != MAIN_LABEL || !main_visible(app) {
        return;
    }
    let Some(ssh_window) = app.get_webview_window(ssh_label) else { return };
    let copy = app.state::<LauncherState>().copy.lock().unwrap().clone();
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
    let handle = app.clone();
    app.dialog()
        .message(copy.hide_prompt)
        .title("VelaTerm")
        .buttons(MessageDialogButtons::OkCancelCustom(copy.hide, copy.keep_open))
        .parent(&ssh_window)
        .show(move |hide| {
            if hide {
                hide_main(&handle);
            }
        });
}

#[tauri::command]
pub fn set_hidden_mode_copy(state: tauri::State<LauncherState>, copy: HiddenModeCopy) {
    *state.copy.lock().unwrap() = copy;
}

const LAUNCHER_WIDTH: f64 = 480.0;
const LAUNCHER_HEIGHT: f64 = 320.0;
const LAUNCHER_EXPANDED_HEIGHT: f64 = 600.0;

pub(crate) fn launcher_height(expanded: bool) -> f64 {
    if expanded {
        LAUNCHER_EXPANDED_HEIGHT
    } else {
        LAUNCHER_HEIGHT
    }
}

pub(crate) fn open_launcher(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LAUNCHER_LABEL, WebviewUrl::App("index.html".into()))
        .title("VelaTerm")
        .inner_size(LAUNCHER_WIDTH, LAUNCHER_HEIGHT)
        .resizable(false)
        .maximizable(false)
        .center()
        .theme(crate::native_theme(app))
        .initialization_script("window.__VLX_LAUNCHER__=true;")
        .disable_drag_drop_handler()
        .build()?;
    // Programmatic closes use destroy(), which skips CloseRequested, so this only sees the user's X.
    let handle = app.clone();
    win.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            quit_without_prompt(&handle);
        }
    });
    Ok(())
}

#[tauri::command]
pub fn launcher_choose_local(app: AppHandle) {
    reveal_main(&app);
    if let Some(win) = app.get_webview_window(LAUNCHER_LABEL) {
        let _ = win.destroy();
    }
}

#[tauri::command]
pub fn launcher_resize(window: WebviewWindow, expanded: bool) -> Result<(), String> {
    if window.label() != LAUNCHER_LABEL {
        return Err("launcher_resize is only available to the launcher window".into());
    }
    window
        .set_size(LogicalSize::new(LAUNCHER_WIDTH, launcher_height(expanded)))
        .map_err(|e| e.to_string())?;
    window.center().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_internal_relaunch(app: AppHandle) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    write_relaunch_marker(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_internal_relaunch(app: AppHandle) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    clear_relaunch_marker(&dir);
    Ok(())
}

const QUIT_SILENT_WATCHDOG: Duration = Duration::from_millis(3000);

/// Quit while main is hidden: main's webview still saves the workspace per the remembered preference and then
/// confirms, but no dialog is drawn. A dead webview never confirms, so the watchdog exits regardless.
pub(crate) fn quit_without_prompt(app: &AppHandle) {
    app.state::<LauncherState>().exiting.store(true, Ordering::SeqCst);
    let _ = app.emit_to(MAIN_LABEL, "app://quit-silent", ());
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(QUIT_SILENT_WATCHDOG);
        app.state::<crate::QuitState>().confirmed.store(true, Ordering::SeqCst);
        app.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn remote_labels_match_every_connection_prefix() {
        for l in ["ssh-1a2b3c4d", "wsl-1a2b3c4d", "remote-x", "account-remote-x"] {
            assert!(is_remote_label(l));
        }
        for l in ["main", "launcher", "screenshot", "sshx"] {
            assert!(!is_remote_label(l));
        }
    }

    #[test]
    fn destroyed_window_is_not_counted() {
        let labels = ["main", "ssh-a", "ssh-b"];
        assert_eq!(remote_count_excluding(labels.iter().copied(), "ssh-a"), 1);
        assert_eq!(remote_count_excluding(["main", "ssh-a"].iter().copied(), "ssh-a"), 0);
    }

    #[test]
    fn launcher_shows_only_on_plain_launch() {
        assert!(should_show_launcher(false, false));
        assert!(!should_show_launcher(true, false));
        assert!(!should_show_launcher(false, true));
    }

    #[test]
    fn relaunch_marker_is_one_shot_and_expires() {
        let dir = temp_dir();
        write_relaunch_marker(&dir).unwrap();
        let now = SystemTime::now();
        assert!(consume_relaunch_marker(&dir, now));
        assert!(!consume_relaunch_marker(&dir, now));
        write_relaunch_marker(&dir).unwrap();
        assert!(!consume_relaunch_marker(&dir, now + RELAUNCH_MARKER_TTL + Duration::from_secs(1)));
        assert!(!dir.join(RELAUNCH_MARKER).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn launcher_grows_only_for_the_expanded_form() {
        assert_eq!(launcher_height(false), 320.0);
        assert_eq!(launcher_height(true), 600.0);
    }

    #[test]
    fn return_prompt_only_for_last_remote_with_hidden_main() {
        assert!(should_offer_return(0, false, false));
        assert!(!should_offer_return(1, false, false));
        assert!(!should_offer_return(0, true, false));
        assert!(!should_offer_return(0, false, true));
    }

    #[test]
    fn hidden_mode_copy_deserializes_from_camel_case() {
        let copy: HiddenModeCopy = serde_json::from_str(
            r#"{"hidePrompt":"a","hide":"b","keepOpen":"c","lastClosedPrompt":"d","returnToLocal":"e","quit":"f"}"#,
        )
        .unwrap();
        assert_eq!(copy.hide_prompt, "a");
        assert_eq!(copy.keep_open, "c");
        assert_eq!(copy.last_closed_prompt, "d");
        assert_eq!(copy.return_to_local, "e");
    }

    #[test]
    fn focus_prefers_visible_main_then_launcher_then_newest_remote() {
        let r = vec!["ssh-old".to_string(), "ssh-new".to_string()];
        assert_eq!(pick_focus_target(true, false, &r), FocusTarget::Main);
        assert_eq!(pick_focus_target(false, true, &r), FocusTarget::Launcher);
        assert_eq!(pick_focus_target(false, false, &r), FocusTarget::Remote("ssh-new".into()));
        assert_eq!(pick_focus_target(false, false, &[]), FocusTarget::Main);
    }
}
