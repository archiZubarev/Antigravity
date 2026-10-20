// Tauri IPC API communication layer
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export interface StateDto {
  is_on: boolean;
  kind: "on" | "off" | "partial" | "off_note" | "blocked";
  note?: string;
}

export interface InstallDto {
  path?: string;
  label: string;
  patched?: boolean;
  manual: boolean;
}

export interface ProviderDto {
  name: string;
  enabled: boolean;
}

export interface HeadlineDto {
  tone: "ok" | "wait" | "fixing" | "action" | "off";
  title: string;
  detail: string;
  action?: "enable_all" | "repair" | "elevate" | "verify";
}

export interface StatusDto {
  admin: boolean;
  installs: InstallDto[];
  client_patch: StateDto;
  watchdog: StateDto;
  dns: StateDto;
  local_proxy: StateDto;
  own_proxy: StateDto;
  builtin_exits: StateDto;
  group_proxy: StateDto;
  dns_rotation: StateDto;
  verify_tls: StateDto;
  own_only: StateDto;
  vpn?: string;
  own_proxy_text: string;
  relay_outdated: boolean;
  relay_running: boolean;
  rules: boolean;
  providers: ProviderDto[];
  relay_egress?: string;
  app_management: boolean;
  headline?: HeadlineDto;
}

export interface LogItemDto {
  level: "step" | "info" | "ok" | "warn" | "err";
  text: string;
}

export interface InitialStateDto {
  version: string;
  pc_code: string;
  settings_own_proxy: string;
  settings_group_key: string;
}

export interface ReportOutcomeDto {
  text: string;
  saved_path?: string;
}

export const api = {
  getInitialState: () => invoke<InitialStateDto>("get_initial_state"),
  getStatus: () => invoke<StatusDto | null>("get_status"),
  getLogs: () => invoke<LogItemDto[]>("get_logs"),
  getBusy: () => invoke<string | null>("get_busy"),
  toggleCap: (cap: string, enable: boolean) => invoke("toggle_cap", { cap, enable }),
  enableAll: () => invoke("enable_all"),
  repair: () => invoke("repair"),
  setOwnProxy: (proxy: string) => invoke("set_own_proxy", { proxy }),
  setGroupKey: (key: string) => invoke("set_group_key", { key }),
  setProvider: (name: string, enabled: boolean) => invoke("set_provider", { name, enabled }),
  reorderProviders: (order: string[]) => invoke("reorder_providers", { order }),
  addPath: (path: string) => invoke("add_path", { path }),
  forgetPath: (path: string) => invoke("forget_path", { path }),
  deepRefresh: () => invoke("deep_refresh"),
  remeasureVpn: () => invoke("remeasure_vpn"),
  openSystemSettings: () => invoke("open_system_settings"),
  openUrl: (url: string) => invoke("open_url", { url }),
  saveReport: () => invoke<ReportOutcomeDto>("save_report"),
  restartClient: () => invoke("restart_client"),

  onStatus: (cb: (status: StatusDto) => void): Promise<UnlistenFn> =>
    listen<StatusDto>("ops_status", (event) => cb(event.payload)),

  onLog: (cb: (log: LogItemDto) => void): Promise<UnlistenFn> =>
    listen<LogItemDto>("ops_log", (event) => cb(event.payload)),

  onBusy: (cb: (busy: string | null) => void): Promise<UnlistenFn> =>
    listen<string | null>("ops_busy", (event) => cb(event.payload)),

  onUpdate: (cb: (version: string) => void): Promise<UnlistenFn> =>
    listen<string>("update_available", (event) => cb(event.payload)),
};
