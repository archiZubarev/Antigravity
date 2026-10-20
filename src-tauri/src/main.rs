#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;

fn main() {
    if env::args().any(|a| a == ag_unlocker_lib::background::FORWARDER_FLAG) {
        ag_unlocker_lib::dns_forwarder::detach_console();
        ag_unlocker_lib::watchdog::start();
        if let Err(e) = ag_unlocker_lib::dns_forwarder::run() {
            ag_unlocker_lib::dns_forwarder::log_fatal(&e);
            std::process::exit(1);
        }
        return;
    }

    if env::args().any(|a| a == ag_unlocker_lib::background::WATCHDOG_FLAG) {
        ag_unlocker_lib::dns_forwarder::detach_console();
        ag_unlocker_lib::watchdog::run_forever();
        return;
    }

    if env::args().any(|a| a == ag_unlocker_lib::background::PROXY_FLAG) {
        ag_unlocker_lib::dns_forwarder::detach_console();
        if let Err(e) = ag_unlocker_lib::dns_forwarder::run_proxy_service() {
            ag_unlocker_lib::dns_forwarder::log_fatal(&e);
            std::process::exit(1);
        }
        return;
    }

    {
        let args: Vec<String> = env::args().collect();
        if let Some(i) = args.iter().position(|a| a == ag_unlocker_lib::elevate::PATCH_ROOT_FLAG) {
            ag_unlocker_lib::elevate::run_helper(args.get(i + 1).cloned());
        }
    }

    ag_unlocker_lib::canary::handle_cli_flags();

    if ag_unlocker_lib::tui::requested() {
        if let Err(e) = ag_unlocker_lib::tui::run(None) {
            ag_unlocker_lib::utils::message_box("Antigravity Unlocker", &e);
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(target_os = "windows"))]
    if ag_unlocker_lib::tui::only_terminal() {
        if let Err(e) = ag_unlocker_lib::tui::run(None) {
            ag_unlocker_lib::utils::message_box("Antigravity Unlocker", &e);
            std::process::exit(1);
        }
        return;
    }

    // Default: run Tauri GUI!
    ag_unlocker_lib::run();
}
