//! Installation detection, path resolution and patching procedures for Antigravity.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::asar::{self, extract_asar};
use crate::patch_binary::{self, patch_all_binaries};
use crate::patch_ide::{is_new_desktop_architecture, patch_desktop, patch_extension_js, patch_ide};
use crate::{background, endpoint, proxy, utils};

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

pub fn resources_dir(install: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        install.join("Resources")
    } else {
        install.join("resources")
    }
}

fn is_install_root(path: &Path) -> bool {
    if !path.exists() || !path.is_dir() {
        return false;
    }

    let path_str = path.to_string_lossy().to_lowercase();
    if path_str == "c:\\windows"
        || path_str.starts_with("c:\\windows\\")
        || path_str.contains("\\windows\\system32")
        || path_str.contains("\\windows\\syswow64")
    {
        return false;
    }

    let resources = resources_dir(path);
    if resources.exists() && resources.is_dir() {
        if resources.join("app.asar").exists()
            || resources.join("app").exists()
            || resources.join("bin").exists()
        {
            return true;
        }
    }
    if path.join("agy.exe").is_file() || path.join("agy").is_file() {
        return true;
    }
    if path.join("Antigravity.exe").is_file()
        || path.join("Antigravity IDE.exe").is_file()
        || path.join("antigravity.exe").is_file()
    {
        return true;
    }
    #[cfg(not(target_os = "windows"))]
    if path.join("antigravity").is_file()
        || path.join("Antigravity").is_file()
        || path.join("antigravity-ide").is_file()
    {
        return true;
    }
    if path.join("out").join("main.js").exists() || path.join("dist").join("main.js").exists() {
        return true;
    }
    false
}

pub fn resolve_install_root(raw: &Path) -> Option<PathBuf> {
    let mut p = raw.to_path_buf();

    if p.is_file() {
        if let Some(parent) = p.parent() {
            p = parent.to_path_buf();
        }
    }

    if !p.exists() {
        return None;
    }

    if is_install_root(&p) {
        return Some(p);
    }

    let mut current = p.clone();
    for _ in 0..4 {
        if let Some(parent) = current.parent() {
            if is_install_root(parent) {
                return Some(parent.to_path_buf());
            }
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    let subfolder_candidates = [
        "Antigravity IDE",
        "Antigravity",
        "agy",
        "Programs\\Antigravity IDE",
        "Programs\\Antigravity",
        "resources",
        "Contents",
    ];
    for sub in subfolder_candidates {
        let candidate = p.join(sub);
        if is_install_root(&candidate) {
            return Some(candidate);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn standard_install_candidates() -> Vec<PathBuf> {
    let local_appdata = env::var("LOCALAPPDATA").unwrap_or_default();
    let prog_files = env::var("PROGRAMFILES").unwrap_or_default();
    let prog_files_x86 = env::var("PROGRAMFILES(X86)").unwrap_or_default();

    vec![
        PathBuf::from(&local_appdata)
            .join("Programs")
            .join("Antigravity"),
        PathBuf::from(&local_appdata)
            .join("Programs")
            .join("Antigravity IDE"),
        PathBuf::from(&prog_files).join("Antigravity"),
        PathBuf::from(&prog_files).join("Antigravity IDE"),
        PathBuf::from(&prog_files_x86).join("Antigravity"),
        PathBuf::from(&prog_files_x86).join("Antigravity IDE"),
        PathBuf::from(&local_appdata).join("Antigravity"),
        PathBuf::from(&local_appdata).join("Antigravity IDE"),
        PathBuf::from(&local_appdata).join("agy").join("bin"),
        PathBuf::from(&local_appdata).join("agy"),
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn standard_install_candidates() -> Vec<PathBuf> {
    let home = env::var("HOME").unwrap_or_default();
    let mut v: Vec<PathBuf> = Vec::new();
    for base in ["/opt", "/usr/share", "/usr/lib", "/usr/local/share"] {
        for name in [
            "Antigravity",
            "Antigravity IDE",
            "antigravity",
            "antigravity-ide",
        ] {
            v.push(PathBuf::from(base).join(name));
        }
    }
    if !home.is_empty() {
        let h = PathBuf::from(&home);
        v.push(h.join(".local/share/Antigravity"));
        v.push(h.join(".local/share/Antigravity IDE"));
        v.push(h.join("Antigravity"));
        v.push(h.join("Antigravity IDE"));
    }
    v
}

#[cfg(target_os = "macos")]
fn standard_install_candidates() -> Vec<PathBuf> {
    let mut v = scan_antigravity_dirs();
    let home = env::var("HOME").unwrap_or_default();
    if !home.is_empty() {
        v.push(PathBuf::from(format!("{}/.local/bin", home)));
        v.push(PathBuf::from(format!("{}/.agy/bin", home)));
    }
    v
}

#[cfg(target_os = "macos")]
fn mac_app_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Ok(home) = env::var("HOME") {
        if !home.is_empty() {
            dirs.push(PathBuf::from(home).join("Applications"));
        }
    }
    dirs
}

#[cfg(target_os = "macos")]
fn scan_antigravity_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for base in mac_app_dirs() {
        let Ok(entries) = fs::read_dir(&base) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.contains("antigravity") && name.ends_with(".app") {
                out.push(e.path().join("Contents"));
            }
        }
    }
    out
}

#[cfg(not(target_os = "windows"))]
fn find_agy_dir() -> Option<PathBuf> {
    let home = env::var("HOME").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(path) = env::var("PATH") {
        dirs.extend(path.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    }
    dirs.push(PathBuf::from(format!("{}/.local/bin", home)));
    dirs.push(PathBuf::from(format!("{}/.agy/bin", home)));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs.push(PathBuf::from("/usr/bin"));
    dirs.into_iter()
        .find(|d| !is_snap_path(d) && d.join("agy").is_file())
}

#[cfg(not(target_os = "windows"))]
fn is_snap_path(p: &Path) -> bool {
    p.starts_with("/snap") || p.starts_with("/var/lib/snapd")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn scan_antigravity_dirs() -> Vec<PathBuf> {
    let home = env::var("HOME").unwrap_or_default();
    let mut out = Vec::new();
    let bases = [
        "/opt".to_string(),
        "/usr/share".to_string(),
        "/usr/lib".to_string(),
        "/usr/local/share".to_string(),
        format!("{}/.local/share", home),
    ];
    for base in bases {
        if let Ok(entries) = fs::read_dir(&base) {
            for e in entries.flatten() {
                let p = e.path();
                let hit = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.to_lowercase().contains("antigravity"));
                if hit && p.is_dir() {
                    out.push(p);
                }
            }
        }
    }
    out
}

pub fn discover_installs_fast() -> Vec<PathBuf> {
    let mut installs = Vec::new();
    for cand in standard_install_candidates() {
        if let Some(resolved) = resolve_install_root(&cand) {
            if !installs.contains(&resolved) {
                installs.push(resolved);
            }
        }
    }
    installs
}

pub fn find_all_installs() -> Vec<PathBuf> {
    let mut installs = Vec::new();
    let mut candidates = standard_install_candidates();

    #[cfg(not(target_os = "windows"))]
    {
        candidates.extend(scan_antigravity_dirs());
        if let Some(agy_dir) = find_agy_dir() {
            candidates.push(agy_dir);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let ps_cmd = r#"Get-ItemProperty HKLM:\Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*, HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*, HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\* | Where-Object { $_.DisplayName -like '*Antigravity*' -or $_.DisplayName -like '*agy*' -or $_.InstallLocation -like '*Antigravity*' } | ForEach-Object { $_.InstallLocation }"#;
        if let Some(output) = utils::powershell_within(ps_cmd, Duration::from_secs(20)) {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let cleaned = clean_input_path(line);
                    if !cleaned.is_empty() {
                        candidates.push(PathBuf::from(&cleaned));
                    }
                }
            }
        }
    }

    for cand in candidates {
        if let Some(resolved) = resolve_install_root(&cand) {
            if !installs.contains(&resolved) {
                installs.push(resolved);
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    installs.retain(|p| !is_snap_path(p));
    installs
}

pub fn restore_pristine_asar(resources: &Path) -> Result<(), String> {
    let app_dir = resources.join("app");
    let app_asar = resources.join("app.asar");
    let asar_bak = resources.join("app.asar.bak");

    if !asar_bak.exists() && !app_asar.exists() {
        return Ok(());
    }

    if app_dir.exists() {
        fs::remove_dir_all(&app_dir)
            .map_err(|e| format!("не удалось удалить resources\\app: {}", e))?;
    }
    if asar_bak.exists() && !app_asar.exists() {
        fs::rename(&asar_bak, &app_asar)
            .map_err(|e| format!("не удалось восстановить app.asar: {}", e))?;
    }
    Ok(())
}

pub struct InstallOutcome {
    pub label: &'static str,
    pub warnings: Vec<String>,
    pub summary: patch_binary::BinarySummary,
}

pub fn process_install(install: &Path) -> Result<InstallOutcome, String> {
    let bin_summary = patch_all_binaries(install);
    let mut warnings: Vec<String> = Vec::new();

    let resources = resources_dir(install);
    let app_dir = resources.join("app");
    let app_asar = resources.join("app.asar");

    if app_asar.exists() {
        let is_new_arch = asar::read_asar_entry(&app_asar, "dist/main.js")
            .and_then(|b| String::from_utf8(b).ok())
            .map_or(false, |src| is_new_desktop_architecture(&src));

        if is_new_arch {
            restore_pristine_asar(&resources)?;
            if bin_summary.ok == 0 {
                return Err(binary_failure_message(&bin_summary));
            }
            return Ok(InstallOutcome {
                label: "Antigravity Desktop",
                warnings,
                summary: bin_summary,
            });
        }

        if app_dir.exists() {
            let _ = fs::remove_dir_all(&app_dir);
        }
        if !extract_asar(&app_asar, &app_dir) {
            return Err("Ошибка получения доступа к приложению".to_string());
        }
    }

    let ide_js = app_dir.join("out").join("main.js");
    let desktop_js = app_dir.join("dist").join("main.js");

    if ide_js.exists() {
        patch_ide(install, &ide_js)?;
        if let Err(e) = patch_extension_js(install) {
            let _ = e;
        }
        if let Err(e) = endpoint::remove_ide(install) {
            warnings.push(format!("Прежний оверрайд эндпоинта не снят: {}", e));
        }
        return Ok(InstallOutcome {
            label: "Antigravity IDE",
            warnings,
            summary: bin_summary,
        });
    } else if desktop_js.exists() {
        let js_patched = patch_desktop(install, &desktop_js)?;
        if !js_patched {
            restore_pristine_asar(&resources)?;
        }
        return Ok(InstallOutcome {
            label: "Antigravity Desktop",
            warnings,
            summary: bin_summary,
        });
    } else if install.join("agy.exe").is_file() || install.join("agy").is_file() {
        if bin_summary.ok == 0 {
            return Err(binary_failure_message(&bin_summary));
        }
        if let Err(e) = endpoint::remove_cli() {
            warnings.push(format!("Прежний {} не снят: {}", endpoint::CLI_ENV_VAR, e));
        }
        return Ok(InstallOutcome {
            label: "Antigravity CLI",
            warnings,
            summary: bin_summary,
        });
    }

    Err("Компоненты приложения не найдены".to_string())
}

fn binary_failure_message(summary: &patch_binary::BinarySummary) -> String {
    if summary.total() == 0 {
        "Бинарник Language Server / CLI не найден в этой установке".to_string()
    } else if let Some(err) = &summary.last_error {
        err.clone()
    } else {
        "Сигнатура в Language Server не найдена — вероятно, вышла новая версия Antigravity"
            .to_string()
    }
}

pub fn remove_legacy_ca_quiet() -> bool {
    let cert = proxy::ca_cert_path();
    if !cert.exists() {
        return false;
    }
    if background::relay_is_outdated() {
        return false;
    }
    endpoint::clear_node_ca(&cert.to_string_lossy()).ok();
    proxy::untrust_ca();
    true
}

pub fn install_label(install: &Path) -> &'static str {
    if install.join("agy.exe").is_file() || install.join("agy").is_file() {
        "Antigravity CLI"
    } else if install.join("Antigravity IDE.exe").exists()
        || resources_dir(install).join("app").join("out").exists()
    {
        "Antigravity IDE"
    } else {
        "Antigravity 2.0"
    }
}
