import React, { useEffect, useState } from "react";
import {
  api,
  InitialStateDto,
  LogItemDto,
  ReportOutcomeDto,
  StatusDto,
} from "./api";
import { Header } from "./components/Header";
import { StatusHero } from "./components/StatusHero";
import { CoreSwitches } from "./components/CoreSwitches";
import { Installations } from "./components/Installations";
import { AdvancedSection } from "./components/AdvancedSection";
import { LogViewer } from "./components/LogViewer";
import { Footer } from "./components/Footer";
import { Icon } from "./icons";

const defaultStateDto = { is_on: false, kind: "off" as const };

export const App: React.FC = () => {
  const [initialState, setInitialState] = useState<InitialStateDto>({
    version: "",
    pc_code: "",
    settings_own_proxy: "",
    settings_group_key: "",
  });
  const [status, setStatus] = useState<StatusDto | null>(null);
  const [logs, setLogs] = useState<LogItemDto[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [updateAvailable, setUpdateAvailable] = useState<string | null>(null);

  useEffect(() => {
    // Initial fetch
    api.getInitialState().then(setInitialState).catch(console.error);
    api.getStatus().then(setStatus).catch(console.error);
    api.getLogs().then(setLogs).catch(console.error);
    api.getBusy().then(setBusy).catch(console.error);

    // Event subscriptions
    const unlisteners: Array<() => void> = [];

    api.onStatus((s) => setStatus(s)).then((unlisten) => unlisteners.push(unlisten));
    api.onLog((l) => setLogs((prev) => [...prev, l])).then((unlisten) => unlisteners.push(unlisten));
    api.onBusy((b) => setBusy(b)).then((unlisten) => unlisteners.push(unlisten));
    api.onUpdate((v) => setUpdateAvailable(v)).then((unlisten) => unlisteners.push(unlisten));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  const handleToggle = (cap: string, enable: boolean) => {
    api.toggleCap(cap, enable).catch(console.error);
  };

  const handleEnableAll = () => {
    api.enableAll().catch(console.error);
  };

  const handleRepair = () => {
    api.repair().catch(console.error);
  };

  const handleRestartClient = () => {
    api.restartClient().catch(console.error);
  };

  const handleOpenSettings = () => {
    api.openSystemSettings().catch(console.error);
  };

  const handleAddPath = async (path: string) => {
    await api.addPath(path);
  };

  const handleForgetPath = (path: string) => {
    api.forgetPath(path).catch(console.error);
  };

  const handleSetOwnProxy = (proxy: string) => {
    api.setOwnProxy(proxy).catch(console.error);
  };

  const handleSetProvider = (name: string, enabled: boolean) => {
    api.setProvider(name, enabled).catch(console.error);
  };

  const handleRemeasureVpn = () => {
    api.remeasureVpn().catch(console.error);
  };

  const handleRefresh = () => {
    api.deepRefresh().catch(console.error);
  };

  const handleSaveReport = async (): Promise<ReportOutcomeDto> => {
    return await api.saveReport();
  };

  const isBusy = Boolean(busy);

  return (
    <div className="app-shell">
      <Header version={initialState.version} busy={busy} />

      <main className="main-content">
        {updateAvailable && (
          <div
            style={{
              padding: "8px 12px",
              background: "rgba(53, 116, 240, 0.15)",
              border: "1px solid var(--accent)",
              borderRadius: 8,
              fontSize: 12,
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
            }}
          >
            <span style={{ display: "flex", alignItems: "center", gap: 6, color: "var(--accent-text)" }}>
              <Icon name="alert-circle" size={14} />
              Доступно обновление Antigravity Unlocker v{updateAvailable}
            </span>
          </div>
        )}

        <StatusHero
          headline={status?.headline}
          busy={busy}
          onEnableAll={handleEnableAll}
          onRepair={handleRepair}
          onOpenSettings={handleOpenSettings}
          onRestartClient={handleRestartClient}
        />

        <CoreSwitches
          clientPatch={status?.client_patch || defaultStateDto}
          watchdog={status?.watchdog || defaultStateDto}
          dns={status?.dns || defaultStateDto}
          localProxy={status?.local_proxy || defaultStateDto}
          builtinExits={status?.builtin_exits || defaultStateDto}
          busy={isBusy}
          onToggle={handleToggle}
        />

        <Installations
          installs={status?.installs || []}
          busy={isBusy}
          onAddPath={handleAddPath}
          onForgetPath={handleForgetPath}
        />

        <AdvancedSection
          status={status}
          initialOwnProxy={initialState.settings_own_proxy}
          busy={isBusy}
          onToggle={handleToggle}
          onSetOwnProxy={handleSetOwnProxy}
          onSetProvider={handleSetProvider}
          onRemeasureVpn={handleRemeasureVpn}
        />

        <LogViewer logs={logs} />
      </main>

      <Footer
        pcCode={initialState.pc_code}
        busy={isBusy}
        onRefresh={handleRefresh}
        onSaveReport={handleSaveReport}
      />
    </div>
  );
};

export default App;
