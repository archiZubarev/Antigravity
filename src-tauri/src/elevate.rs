//! Patching an install this user may not write: root-owned Linux packages
//! (Arch puts Antigravity under `/opt` or `/usr/lib`), where the patch's
//! temp-then-rename fails with `Permission denied` before a byte is written.
//!
//! Only the write is elevated. The window, the service and every per-user file
//! (`~/.local/share/agunlocker`, the systemd user unit, `environment.d`) stay
//! with the user: run as a whole under `sudo`, all of those land in root's home
//! and the service is root's, not theirs. So the window asks polkit to start
//! this same exe as root with `--patch-root <install>`, which patches that one
//! install and exits, and reads what it did back from its stdout.
//!
//! macOS adds a second refusal that root does not lift. Since Ventura a program
//! may change another app's bundle only with the user's leave under
//! «Управление приложениями» (App Management), and without it the write fails
//! with `EPERM` - for root as much as for anyone. A root-owned bundle fails with
//! `EACCES` instead. The two need opposite answers, so they are told apart by
//! the error and never by ownership: a password for the first would be asked
//! for nothing.
//!
//! A field report proposed the other shape - the whole GUI under sudo, the
//! install's permissions loosened for the session and restored on close. A
//! crash leaves them loosened, and the per-user state above is wrong for the
//! whole session, so it was not taken.

use std::path::Path;
#[cfg(not(target_os = "windows"))]
use std::path::PathBuf;

/// The helper's flag: patch the install that follows, print the result, exit.
pub const PATCH_ROOT_FLAG: &str = "--patch-root";

/// What the helper reported for its install.
#[derive(Debug, Default)]
pub struct Elevated {
    pub label: String,
    pub warnings: Vec<String>,
    pub proxy_var: bool,
    pub proxy_var_retryable: bool,
}

/// Whether patching `inst` needs root: a directory the patch writes into
/// refuses this user. Never for root itself, and never on Windows, where the
/// window is elevated already.
pub fn needs_root(inst: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = inst;
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        !crate::utils::is_admin()
            && written_dirs(inst)
                .iter()
                .any(|d| probe(d) == Probe::NeedsRoot)
    }
}

/// macOS: whether App Management stands between this program and `inst`.
/// Root would be refused the same way, so this is asked before `needs_root`.
#[cfg(target_os = "macos")]
pub fn blocked_by_app_management(inst: &Path) -> bool {
    written_dirs(inst)
        .iter()
        .any(|d| probe(d) == Probe::AppManagement)
}

/// The app a user has to allow under App Management: this bundle when the
/// window runs from one, otherwise whatever started a bare binary - in
/// practice the terminal.
#[cfg(target_os = "macos")]
pub fn app_management_holder() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.ancestors()
                .find(|p| p.extension().is_some_and(|e| e == "app"))
                .and_then(|app| app.file_stem().map(|s| s.to_string_lossy().into_owned()))
        })
        .map(|name| format!("«{name}»"))
        .unwrap_or_else(|| "терминал, из которого запущен анлокер,".to_string())
}

/// The settings pane that holds the App Management list.
#[cfg(target_os = "macos")]
pub const APP_MANAGEMENT_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AppBundles";

/// The directories the patch creates its temps in: beside each binary it
/// would rewrite, and beside the JS it would rewrite.
#[cfg(not(target_os = "windows"))]
fn written_dirs(inst: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = crate::patch_binary::binary_targets(inst)
        .into_iter()
        .filter(|t| t.is_file())
        .filter_map(|t| t.parent().map(Path::to_path_buf))
        .collect();
    let app = crate::resources_dir(inst).join("app");
    for js in [
        app.join("out").join("main.js"),
        app.join("dist").join("main.js"),
    ] {
        if let Some(dir) = js.parent().filter(|_| js.is_file()) {
            dirs.push(dir.to_path_buf());
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

/// What a write into a directory would meet.
#[cfg(not(target_os = "windows"))]
#[derive(Debug, PartialEq, Eq)]
enum Probe {
    /// Writable - or failing for a reason the patch itself will report.
    Fine,
    /// Refused to this user: root would be let in.
    NeedsRoot,
    /// macOS App Management: refused to root as well, and only the user's
    /// leave in System Settings opens it.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    AppManagement,
}

/// Asked by trying, not by mode bits: ACLs, read-only mounts and ownership all
/// answer the same question the rename will ask. Only a refusal counts - any
/// other failure is left for the patch itself to report.
#[cfg(not(target_os = "windows"))]
fn probe(dir: &Path) -> Probe {
    let probe = dir.join(format!(".ag_write_probe_{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            Probe::Fine
        }
        // EPERM, not EACCES: the sandbox-style refusal App Management gives.
        // On Linux an EPERM here means the same as EACCES, so it stays there.
        Err(e) if cfg!(target_os = "macos") && e.raw_os_error() == Some(1) => Probe::AppManagement,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Probe::NeedsRoot,
        Err(_) => Probe::Fine,
    }
}

/// The command a user can run by hand when no password prompt can be shown.
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub fn manual_command(inst: &Path) -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "./ag_unlocker".to_string());
    format!(
        "sudo {} {} {}",
        shell_quote(&exe),
        PATCH_ROOT_FLAG,
        shell_quote(&inst.display().to_string())
    )
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// macOS: the system's own administrator prompt, through `osascript`.
///
/// Paths go in as arguments and are quoted by AppleScript's `quoted form`, so a
/// space or a quote in either cannot change the command. `|| true` keeps the
/// helper's report: AppleScript turns a non-zero exit into an error and drops
/// stdout, and the report - `ERR` included - is what says what happened.
#[cfg(target_os = "macos")]
pub fn patch_as_root(inst: &Path) -> Result<Elevated, String> {
    use std::process::Command;
    let exe = std::env::current_exe().map_err(|e| format!("не найден путь к программе: {e}"))?;
    let script = format!(
        "do shell script (quoted form of item 1 of argv) & \" {} \" & (quoted form of item 2 of argv) & \" || true\" with administrator privileges without altering line endings",
        PATCH_ROOT_FLAG
    );
    let out = Command::new("osascript")
        .args(["-e", "on run argv", "-e", &script, "-e", "end run"])
        .arg(&exe)
        .arg(inst)
        .output()
        .map_err(|e| format!("osascript не запустился: {e}"))?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        // -128 is "User canceled" in every AppleScript dialog.
        return Err(if stderr.contains("-128") {
            "пароль не введён — патч не наложен".to_string()
        } else {
            format!("патч от имени администратора не удался: {}", stderr.trim())
        });
    }
    match parse_report(&String::from_utf8_lossy(&out.stdout)) {
        Ok(done) => Ok(done),
        Err(e) if !e.is_empty() => Err(e),
        Err(_) => Err(format!(
            "патч от имени администратора ничего не сообщил. Выполните в терминале: {}",
            manual_command(inst)
        )),
    }
}

/// Patches `inst` through `pkexec`, which shows the desktop's own password
/// prompt. `--disable-internal-agent`: with no desktop agent (a TUI over SSH)
/// pkexec would otherwise ask on the terminal the TUI is drawing on, so it
/// fails instead and the caller shows `manual_command`.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn patch_as_root(inst: &Path) -> Result<Elevated, String> {
    use std::process::Command;
    let exe = std::env::current_exe().map_err(|e| format!("не найден путь к exe: {e}"))?;
    let out = Command::new("pkexec")
        .arg("--disable-internal-agent")
        .arg(&exe)
        .arg(PATCH_ROOT_FLAG)
        .arg(inst)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "установка принадлежит root, а pkexec в системе нет. Выполните в терминале: {}",
                    manual_command(inst)
                )
            } else {
                format!("pkexec не запустился: {e}")
            }
        })?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed = parse_report(&stdout);
    match (out.status.code(), parsed) {
        (Some(0), Ok(done)) => Ok(done),
        (_, Err(e)) if !e.is_empty() => Err(e),
        // pkexec's own codes: 126 - the prompt was dismissed or refused, 127 -
        // no agent to show it.
        (Some(126), _) => Err("пароль не введён — патч не наложен".to_string()),
        (Some(127), _) => Err(format!(
            "окно запроса пароля недоступно. Выполните в терминале: {}",
            manual_command(inst)
        )),
        (code, _) => Err(format!(
            "патч от имени root не удался (код {}): {}",
            code.map_or("—".to_string(), |c| c.to_string()),
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

#[cfg(target_os = "windows")]
pub fn patch_as_root(_inst: &Path) -> Result<Elevated, String> {
    Err("не нужно на Windows".to_string())
}

/// The helper's side, run as root: patch one install, report in lines of
/// `KEY<TAB>value`, exit 0 on success.
pub fn run_helper(arg: Option<String>) -> ! {
    let Some(inst) = arg
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_dir())
    else {
        println!("ERR\tне указана папка установки");
        std::process::exit(2);
    };
    match crate::process_install(&inst) {
        Ok(o) => {
            println!("OK\t{}", one_line(o.label));
            for w in o.warnings {
                println!("WARN\t{}", one_line(&w));
            }
            println!(
                "PROXYVAR\t{}\t{}",
                u8::from(o.summary.proxy_var > 0),
                u8::from(o.summary.proxy_var_retryable)
            );
            std::process::exit(0);
        }
        Err(e) => {
            println!("ERR\t{}", one_line(&e));
            std::process::exit(1);
        }
    }
}

fn one_line(s: &str) -> String {
    s.replace(['\n', '\r', '\t'], " ")
}

/// The helper's lines back into a result. `Err("")` when it said nothing at
/// all - pkexec refused before it ran - so the caller explains by exit code.
fn parse_report(stdout: &str) -> Result<Elevated, String> {
    let mut done = Elevated::default();
    let mut ok = false;
    for line in stdout.lines() {
        let mut parts = line.split('\t');
        match parts.next() {
            Some("OK") => {
                ok = true;
                done.label = parts.next().unwrap_or_default().to_string();
            }
            Some("WARN") => done
                .warnings
                .push(parts.next().unwrap_or_default().to_string()),
            Some("PROXYVAR") => {
                done.proxy_var = parts.next() == Some("1");
                done.proxy_var_retryable = parts.next() == Some("1");
            }
            Some("ERR") => return Err(parts.next().unwrap_or_default().to_string()),
            _ => {}
        }
    }
    if ok {
        Ok(done)
    } else {
        Err(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helpers_report_reads_back() {
        let done = parse_report("OK\tAntigravity IDE\nWARN\tоверрайд не снят\nPROXYVAR\t1\t0\n")
            .expect("ok");
        assert_eq!(done.label, "Antigravity IDE");
        assert_eq!(done.warnings, vec!["оверрайд не снят".to_string()]);
        assert!(done.proxy_var && !done.proxy_var_retryable);
        assert_eq!(
            parse_report("ERR\tСигнатура не найдена\n").err().as_deref(),
            Some("Сигнатура не найдена")
        );
        assert_eq!(parse_report("").err().as_deref(), Some(""));
    }

    /// A binary in a directory this user cannot write is what sends the patch
    /// through pkexec; one in a writable directory is patched in place.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_read_only_install_needs_root_and_a_writable_one_does_not() {
        use std::os::unix::fs::PermissionsExt;
        if crate::utils::is_admin() {
            return; // root writes anywhere; nothing to measure
        }
        let inst = std::env::temp_dir().join(format!("ag_elevate_{}", std::process::id()));
        let bin = crate::resources_dir(&inst).join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("language_server_linux_x64"), b"x").unwrap();
        assert!(!needs_root(&inst));
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert!(needs_root(&inst));
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&inst);
    }

    #[test]
    fn the_manual_command_survives_a_quote_in_the_path() {
        assert_eq!(shell_quote("/opt/it's here"), "'/opt/it'\\''s here'");
    }
}
