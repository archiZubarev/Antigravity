import React from "react";
import { StateDto } from "../api";

interface CoreSwitchesProps {
  clientPatch: StateDto;
  watchdog: StateDto;
  dns: StateDto;
  localProxy: StateDto;
  builtinExits: StateDto;
  busy: boolean;
  onToggle: (cap: string, enable: boolean) => void;
}

export const CoreSwitches: React.FC<CoreSwitchesProps> = ({
  clientPatch,
  watchdog,
  dns,
  localProxy,
  builtinExits,
  busy,
  onToggle,
}) => {
  const switches = [
    {
      id: "client_patch",
      label: "Патч Antigravity",
      desc: "Бинарный патч language_server / agy для снятия региональных блокировок",
      state: clientPatch,
    },
    {
      id: "watchdog",
      label: "Фоновый сторож",
      desc: "LaunchAgent служба: проверяет и удерживает патч при обновлениях клиента",
      state: watchdog,
    },
    {
      id: "dns",
      label: "DNS / DoH обход",
      desc: "Шифрованное разрешение служебных доменов Google и сервисов",
      state: dns,
    },
    {
      id: "local_proxy",
      label: "Локальный прокси (127.0.0.1:53129)",
      desc: "Внутренний прокси-сервер для маршрутизации запросов расширения",
      state: localProxy,
    },
    {
      id: "builtin_exits",
      label: "Резервные шлюзы",
      desc: "Автоматический выбор доступных точек выхода при блокировках",
      state: builtinExits,
    },
  ];

  return (
    <div className="card">
      <div className="card-header">
        <span className="card-title">Основные службы</span>
      </div>

      <div className="switches-list">
        {switches.map((item) => {
          const isBlocked = item.state.kind === "blocked";
          const hasNote = Boolean(item.state.note);
          const noteClass = item.state.kind === "blocked" ? "blocked" : item.state.kind === "off_note" ? "warn" : "";

          return (
            <div key={item.id} className="switch-row">
              <div className="switch-info">
                <span className="switch-label">{item.label}</span>
                <span className={`switch-note ${noteClass}`}>
                  {hasNote ? item.state.note : item.desc}
                </span>
              </div>

              <label className="switch">
                <input
                  type="checkbox"
                  checked={item.state.is_on}
                  disabled={busy || isBlocked}
                  onChange={(e) => onToggle(item.id, e.target.checked)}
                />
                <span className="switch-track" />
              </label>
            </div>
          );
        })}
      </div>
    </div>
  );
};
