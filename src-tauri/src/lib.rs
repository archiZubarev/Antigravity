//! Tauri v2 application library for Antigravity Unlocker.

use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tauri::{Emitter, Manager, State};

pub mod asar;
pub mod background;
pub mod canary;
pub mod dns;
pub mod dns_client;
pub mod dns_forwarder;
pub mod doh;
pub mod egress;
pub mod elevate;
pub mod endpoint;
pub mod gate;
pub mod group;
pub mod group_key;
pub mod gui;
pub mod health;
pub mod hosts_pin;
pub mod hwid;
pub mod loopback;
pub mod ls_log;
pub mod net;
pub mod ops;
pub mod patch_binary;
pub mod patch_ide;
pub mod portcheck;
pub mod proxy;
pub mod resolvers;
pub mod routes;
pub mod settings;
pub mod tui;
pub mod install;
pub mod update;
pub mod upstream;
pub mod utils;
pub mod watchdog;

pub use install::*;

use crate::gui::status::{self, Headline, Tone};
use crate::ops::{Cap, Cmd, Event, Level, Status, Worker};
use crate::settings::Settings;

#[derive(Serialize, Clone, Debug)]
pub struct StateDto {
    pub is_on: bool,
    pub kind: String,
    pub note: Option<String>,
}

impl From<&ops::State> for StateDto {
    fn from(s: &ops::State) -> Self {
        match s {
            ops::State::On => Self {
                is_on: true,
                kind: "on".into(),
                note: None,
            },
            ops::State::Off => Self {
                is_on: false,
                kind: "off".into(),
                note: None,
            },
            ops::State::Partial(n) => Self {
                is_on: true,
                kind: "partial".into(),
                note: Some(n.clone()),
            },
            ops::State::OffNote(n) => Self {
                is_on: false,
                kind: "off_note".into(),
                note: Some(n.clone()),
            },
            ops::State::Blocked(n) => Self {
                is_on: false,
                kind: "blocked".into(),
                note: Some(n.clone()),
            },
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct InstallDto {
    pub path: Option<String>,
    pub label: String,
    pub patched: Option<bool>,
    pub manual: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct ProviderDto {
    pub name: String,
    pub enabled: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct HeadlineDto {
    pub tone: String,
    pub title: String,
    pub detail: String,
    pub action: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct StatusDto {
    pub admin: bool,
    pub installs: Vec<InstallDto>,
    pub client_patch: StateDto,
    pub watchdog: StateDto,
    pub dns: StateDto,
    pub local_proxy: StateDto,
    pub own_proxy: StateDto,
    pub builtin_exits: StateDto,
    pub group_proxy: StateDto,
    pub dns_rotation: StateDto,
    pub verify_tls: StateDto,
    pub own_only: StateDto,
    pub vpn: Option<String>,
    pub own_proxy_text: String,
    pub relay_outdated: bool,
    pub relay_running: bool,
    pub rules: bool,
    pub providers: Vec<ProviderDto>,
    pub relay_egress: Option<String>,
    pub app_management: bool,
    pub headline: Option<HeadlineDto>,
}

#[derive(Serialize, Clone, Debug)]
pub struct GateDto {
    pub newest_refusal_ago: Option<u64>,
    pub answered_ago: Option<u64>,
    pub answered_count: u32,
    pub route: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct LogItemDto {
    pub level: String,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct InitialStateDto {
    pub version: String,
    pub pc_code: String,
    pub settings_own_proxy: String,
    pub settings_group_key: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ReportOutcomeDto {
    pub text: String,
    pub saved_path: Option<String>,
}

pub struct AppSharedState {
    pub worker: Worker,
    pub status: Mutex<Option<Status>>,
    pub gate_view: Mutex<gate::View>,
    pub gate_at: Mutex<Instant>,
    pub logs: Mutex<Vec<LogItemDto>>,
    pub busy: Mutex<Option<String>>,
}

fn convert_headline(h: &Headline) -> HeadlineDto {
    HeadlineDto {
        tone: match h.tone {
            Tone::Ok => "ok".into(),
            Tone::Wait => "wait".into(),
            Tone::Fixing => "fixing".into(),
            Tone::Action => "action".into(),
            Tone::Off => "off".into(),
        },
        title: h.title.clone(),
        detail: h.detail.clone(),
        action: h.action.map(|a| match a {
            status::Action::EnableAll => "enable_all".into(),
            status::Action::Repair => "repair".into(),
            status::Action::Elevate => "elevate".into(),
            status::Action::Verify => "verify".into(),
        }),
    }
}

fn make_status_dto(s: &Status, gate: &gate::View, gate_at: Instant) -> StatusDto {
    let facts = status::Facts::read(s, gate, gate_at.elapsed());
    let headline = Some(convert_headline(&status::headline(&facts)));
    StatusDto {
        admin: s.admin,
        installs: s
            .installs
            .iter()
            .map(|i| InstallDto {
                path: i.path.as_ref().map(|p| p.to_string_lossy().to_string()),
                label: i.label.to_string(),
                patched: i.patched,
                manual: i.manual,
            })
            .collect(),
        client_patch: (&s.client_patch).into(),
        watchdog: (&s.watchdog).into(),
        dns: (&s.dns).into(),
        local_proxy: (&s.local_proxy).into(),
        own_proxy: (&s.own_proxy).into(),
        builtin_exits: (&s.builtin_exits).into(),
        group_proxy: (&s.group_proxy).into(),
        dns_rotation: (&s.dns_rotation).into(),
        verify_tls: (&s.verify_tls).into(),
        own_only: (&s.own_only).into(),
        vpn: s.vpn.map(|v| match v {
            ops::VpnSeen::None => "none".into(),
            ops::VpnSeen::NotCarryingClient => "not_carrying_client".into(),
            ops::VpnSeen::CarryingClient => "carrying_client".into(),
            ops::VpnSeen::PartlyCarryingClient => "partly_carrying_client".into(),
            ops::VpnSeen::ViaLocalProxy => "via_local_proxy".into(),
            ops::VpnSeen::Unmeasured => "unmeasured".into(),
        }),
        own_proxy_text: s.own_proxy_text.clone(),
        relay_outdated: s.relay_outdated,
        relay_running: s.relay_running,
        rules: s.rules,
        providers: s
            .providers
            .iter()
            .map(|p| ProviderDto {
                name: p.name.clone(),
                enabled: p.enabled,
            })
            .collect(),
        relay_egress: s.relay_egress.as_ref().map(|e| match e {
            egress::ClientEgress::Physical => "physical",
            egress::ClientEgress::Tunnel => "tunnel",
            egress::ClientEgress::Mixed => "mixed",
            egress::ClientEgress::ViaLocalProxy => "via_local_proxy",
            egress::ClientEgress::Unknown => "unknown",
        }.to_string()),
        app_management: s.app_management,
        headline,
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_initial_state() -> InitialStateDto {
    let settings = Settings::load();
    InitialStateDto {
        version: update::current_version().to_string(),
        pc_code: hwid::pc_code().to_string(),
        settings_own_proxy: settings.own_proxy,
        settings_group_key: settings.group_key,
    }
}

#[tauri::command]
fn get_status(state: State<'_, Arc<AppSharedState>>) -> Option<StatusDto> {
    let s = state.status.lock().ok()?;
    let g = state.gate_view.lock().ok()?;
    let at = state.gate_at.lock().ok().map(|t| *t).unwrap_or_else(Instant::now);
    s.as_ref().map(|status| make_status_dto(status, &g, at))
}

#[tauri::command]
fn get_logs(state: State<'_, Arc<AppSharedState>>) -> Vec<LogItemDto> {
    state.logs.lock().map(|l| l.clone()).unwrap_or_default()
}

#[tauri::command]
fn get_busy(state: State<'_, Arc<AppSharedState>>) -> Option<String> {
    state.busy.lock().ok().and_then(|b| b.clone())
}

#[tauri::command]
fn toggle_cap(cap: String, enable: bool, state: State<'_, Arc<AppSharedState>>) {
    let c = match cap.as_str() {
        "client_patch" => Cap::ClientPatch,
        "watchdog" => Cap::Watchdog,
        "dns" => Cap::Dns,
        "local_proxy" => Cap::LocalProxy,
        "own_proxy" => Cap::OwnProxy,
        "builtin_exits" => Cap::BuiltinExits,
        "group_proxy" => Cap::GroupProxy,
        "dns_rotation" => Cap::DnsRotation,
        "verify_tls" => Cap::VerifyTls,
        "own_only" => Cap::OwnOnly,
        _ => return,
    };
    state.worker.send(Cmd::Set(c, enable));
}

#[tauri::command]
fn enable_all(state: State<'_, Arc<AppSharedState>>) {
    state.worker.send(Cmd::EnableAll);
}

#[tauri::command]
fn repair(state: State<'_, Arc<AppSharedState>>) {
    state.worker.send(Cmd::Repair);
}

#[tauri::command]
fn set_own_proxy(proxy: String, state: State<'_, Arc<AppSharedState>>) {
    state.worker.send(Cmd::SetOwnProxy(proxy));
}

#[tauri::command]
fn set_group_key(key: String, state: State<'_, Arc<AppSharedState>>) {
    state.worker.send(Cmd::SetGroupKey(key));
}

#[tauri::command]
fn set_provider(name: String, enabled: bool, state: State<'_, Arc<AppSharedState>>) {
    state.worker.send(Cmd::SetProvider(name, enabled));
}

#[tauri::command]
fn reorder_providers(order: Vec<String>, state: State<'_, Arc<AppSharedState>>) {
    state.worker.send(Cmd::ReorderProviders(order));
}

#[tauri::command]
fn add_path(path: String, state: State<'_, Arc<AppSharedState>>) -> Result<(), String> {
    let p = PathBuf::from(crate::main_entry::clean_input_path(&path));
    if !p.exists() {
        return Err("Указанный путь не существует".to_string());
    }
    state.worker.send(Cmd::AddPath(p));
    Ok(())
}

#[tauri::command]
fn forget_path(path: String, state: State<'_, Arc<AppSharedState>>) {
    state.worker.send(Cmd::ForgetPath(PathBuf::from(path)));
}

#[tauri::command]
fn deep_refresh(state: State<'_, Arc<AppSharedState>>) {
    state.worker.send(Cmd::DeepRefresh);
}

#[tauri::command]
fn remeasure_vpn(state: State<'_, Arc<AppSharedState>>) {
    state.worker.send(Cmd::RemeasureVpn);
}

#[tauri::command]
fn open_system_settings() {
    #[cfg(target_os = "macos")]
    crate::utils::open_url(crate::elevate::APP_MANAGEMENT_PANE);
    #[cfg(target_os = "windows")]
    crate::utils::open_url("ms-settings:developers");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    crate::utils::open_url("");
}

#[tauri::command]
fn open_url(url: String) {
    crate::utils::open_url(&url);
}

#[tauri::command]
fn save_report(state: State<'_, Arc<AppSharedState>>) -> ReportOutcomeDto {
    let s = state.status.lock().ok().and_then(|guard| guard.clone());
    let g = state
        .gate_view
        .lock()
        .ok()
        .map(|guard| guard.clone())
        .unwrap_or_default();

    let text = crate::gui::report::build(s.as_ref(), &g);
    let saved_path = crate::utils::desktop_dir().and_then(|dir| {
        crate::utils::save_text_file(&dir, crate::utils::REPORT_FILE, &text)
            .map(|p| p.to_string_lossy().to_string())
    });

    ReportOutcomeDto { text, saved_path }
}

#[tauri::command]
fn restart_client() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("killall")
            .arg("Antigravity")
            .output();
        let _ = std::process::Command::new("open")
            .arg("-a")
            .arg("Antigravity")
            .output();
    }
}

// ---------------------------------------------------------------------------
// Helpers re-exported for main & CLI
// ---------------------------------------------------------------------------
pub mod main_entry {
    pub fn clean_input_path(input: &str) -> String {
        let mut s = input.trim();
        if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
            if s.len() >= 2 {
                s = &s[1..s.len() - 1];
            }
        }
        s = s.trim();
        s = s.trim_matches('"').trim_matches('\'').trim();
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Main Tauri Application Runner
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (ev_tx, ev_rx) = channel::<Event>();
    let (gate_tx, gate_rx) = channel::<gate::Signal>();
    let (up_tx, up_rx) = channel::<update::ReleaseInfo>();

    let shared = Arc::new(AppSharedState {
        worker: ops::spawn(ev_tx, Box::new(|| {})),
        status: Mutex::new(None),
        gate_view: Mutex::new(gate::View::default()),
        gate_at: Mutex::new(Instant::now()),
        logs: Mutex::new(Vec::new()),
        busy: Mutex::new(None),
    });

    // Start background watchers
    gate::spawn_watch(gate_tx, Box::new(|| {}));
    update::spawn_watch(up_tx, Box::new(|| {}));

    shared.worker.send(Cmd::Ready);

    let shared_clone = shared.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(shared)
        .setup(move |app| {
            let handle = app.handle().clone();
            let shared_for_threads = shared_clone.clone();

            // Background event receiver for ops worker
            let h1 = handle.clone();
            let s1 = shared_for_threads.clone();
            std::thread::spawn(move || {
                while let Ok(event) = ev_rx.recv() {
                    match event {
                        Event::Log(level, text) => {
                            let lvl_str = match level {
                                Level::Step => "step",
                                Level::Info => "info",
                                Level::Ok => "ok",
                                Level::Warn => "warn",
                                Level::Err => "err",
                            };
                            let item = LogItemDto {
                                level: lvl_str.to_string(),
                                text: text.clone(),
                            };
                            if let Ok(mut l) = s1.logs.lock() {
                                if l.len() >= 400 {
                                    l.remove(0);
                                }
                                l.push(item.clone());
                            }
                            let _ = h1.emit("ops_log", &item);
                        }
                        Event::Status(st) => {
                            let g = s1.gate_view.lock().ok().map(|g| g.clone()).unwrap_or_default();
                            let at = s1.gate_at.lock().ok().map(|t| *t).unwrap_or_else(Instant::now);
                            let dto = make_status_dto(&st, &g, at);
                            if let Ok(mut guard) = s1.status.lock() {
                                *guard = Some(*st);
                            }
                            let _ = h1.emit("ops_status", &dto);
                        }
                        Event::Busy(b) => {
                            if let Ok(mut guard) = s1.busy.lock() {
                                *guard = b.clone();
                            }
                            let _ = h1.emit("ops_busy", &b);
                        }
                    }
                }
            });

            // Gate watcher event receiver
            let h2 = handle.clone();
            let s2 = shared_for_threads.clone();
            std::thread::spawn(move || {
                while let Ok(sig) = gate_rx.recv() {
                    let mut updated_dto: Option<StatusDto> = None;
                    match sig {
                        gate::Signal::Gate(view) => {
                            let now = Instant::now();
                            if let Ok(mut g_guard) = s2.gate_view.lock() {
                                *g_guard = view.clone();
                            }
                            if let Ok(mut at_guard) = s2.gate_at.lock() {
                                *at_guard = now;
                            }
                            if let Ok(st_guard) = s2.status.lock() {
                                if let Some(st) = st_guard.as_ref() {
                                    updated_dto = Some(make_status_dto(st, &view, now));
                                }
                            }
                        }
                        gate::Signal::MeasureVpn => {
                            s2.worker.send(Cmd::RemeasureVpn);
                        }
                    }
                    if let Some(dto) = updated_dto {
                        let _ = h2.emit("ops_status", &dto);
                    }
                }
            });

            // Update watcher event receiver
            let h3 = handle.clone();
            std::thread::spawn(move || {
                while let Ok(info) = up_rx.recv() {
                    let _ = h3.emit("update_available", info.tag_name);
                }
            });

            // Reveal window after slight delay to ensure first paint is ready
            let h_win = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(200));
                if let Some(window) = h_win.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_initial_state,
            get_status,
            get_logs,
            get_busy,
            toggle_cap,
            enable_all,
            repair,
            set_own_proxy,
            set_group_key,
            set_provider,
            reorder_providers,
            add_path,
            forget_path,
            deep_refresh,
            remeasure_vpn,
            open_system_settings,
            open_url,
            save_report,
            restart_client,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
