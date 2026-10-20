import React, { useState } from "react";
import { StatusDto } from "../api";
import { Icon } from "../icons";

interface AdvancedSectionProps {
  status: StatusDto | null;
  initialOwnProxy: string;
  busy: boolean;
  onToggle: (cap: string, enable: boolean) => void;
  onSetOwnProxy: (proxy: string) => void;
  onSetProvider: (name: string, enabled: boolean) => void;
  onRemeasureVpn: () => void;
}

export const AdvancedSection: React.FC<AdvancedSectionProps> = ({
  status,
  initialOwnProxy,
  busy,
  onToggle,
  onSetOwnProxy,
  onSetProvider,
  onRemeasureVpn,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [proxyInput, setProxyInput] = useState(initialOwnProxy);
  const [proxySaved, setProxySaved] = useState(false);

  const handleSaveProxy = (e: React.FormEvent) => {
    e.preventDefault();
    onSetOwnProxy(proxyInput.trim());
    setProxySaved(true);
    setTimeout(() => setProxySaved(false), 2000);
  };

  const vpnStatusMap: Record<string, string> = {
    none: "VPN не обнаружен (прямое подключение)",
    not_carrying_client: "VPN активен (клиент в обход туннеля)",
    carrying_client: "Весь трафик внутри VPN",
    partly_carrying_client: "Часть трафика внутри VPN",
    via_local_proxy: "Трафик через локальный прокси",
    unmeasured: "Не измерялся",
  };

  const egressMap: Record<string, string> = {
    physical: "Физический интерфейс (ISP)",
    tunnel: "Сетевой туннель (VPN)",
    mixed: "Смешанный",
    via_local_proxy: "Локальный шлюз",
    unknown: "Не определен",
  };

  return (
    <div className="card">
      <div className="accordion-header" onClick={() => setIsOpen(!isOpen)}>
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <Icon name="tool" size={13} />
          Тонкая настройка сети
        </span>
        <Icon name={isOpen ? "chevron-up" : "chevron-down"} size={14} />
      </div>

      {isOpen && (
        <div className="accordion-body">
          {/* Custom Proxy */}
          <div style={{ display: "flex", flexDirection: "column", gap: 6, paddingTop: 4 }}>
            <label className="switch-label" style={{ fontSize: 12 }}>
              Собственный прокси-сервер (SOCKS5 / HTTP)
            </label>
            <form onSubmit={handleSaveProxy} style={{ display: "flex", gap: 8 }}>
              <input
                type="text"
                className="input-box"
                placeholder="socks5://127.0.0.1:1080 или http://host:port"
                value={proxyInput}
                onChange={(e) => setProxyInput(e.target.value)}
                disabled={busy}
              />
              <button
                type="submit"
                className="btn sm primary"
                disabled={busy || proxyInput === initialOwnProxy && !proxySaved}
                style={{ flexShrink: 0 }}
              >
                {proxySaved ? "Сохранено" : "Применить"}
              </button>
            </form>
          </div>

          {/* Toggle Switches */}
          {status && (
            <div style={{ display: "flex", flexDirection: "column", gap: 2, marginTop: 4 }}>
              <div className="switch-row">
                <div className="switch-info">
                  <span className="switch-label">Только собственный прокси</span>
                  <span className="switch-note">Не использовать автоматические резервные шлюзы</span>
                </div>
                <label className="switch">
                  <input
                    type="checkbox"
                    checked={status.own_only.is_on}
                    disabled={busy || status.own_only.kind === "blocked"}
                    onChange={(e) => onToggle("own_only", e.target.checked)}
                  />
                  <span className="switch-track" />
                </label>
              </div>

              <div className="switch-row">
                <div className="switch-info">
                  <span className="switch-label">Сверять TLS сертификаты</span>
                  <span className="switch-note">Строгая валидация цепочки доверия upstream</span>
                </div>
                <label className="switch">
                  <input
                    type="checkbox"
                    checked={status.verify_tls.is_on}
                    disabled={busy}
                    onChange={(e) => onToggle("verify_tls", e.target.checked)}
                  />
                  <span className="switch-track" />
                </label>
              </div>

              <div className="switch-row">
                <div className="switch-info">
                  <span className="switch-label">Ротация DNS резолверов</span>
                  <span className="switch-note">Балансировка запросов между активными DNS</span>
                </div>
                <label className="switch">
                  <input
                    type="checkbox"
                    checked={status.dns_rotation.is_on}
                    disabled={busy}
                    onChange={(e) => onToggle("dns_rotation", e.target.checked)}
                  />
                  <span className="switch-track" />
                </label>
              </div>
            </div>
          )}

          {/* Providers List */}
          {status && status.providers && status.providers.length > 0 && (
            <div style={{ marginTop: 6, display: "flex", flexDirection: "column", gap: 6 }}>
              <span className="card-title" style={{ fontSize: 11 }}>DNS / DoH Провайдеры</span>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {status.providers.map((prov) => (
                  <button
                    key={prov.name}
                    className={`btn sm ${prov.enabled ? "primary" : "bordered"}`}
                    onClick={() => onSetProvider(prov.name, !prov.enabled)}
                    disabled={busy}
                    style={{ fontSize: 11 }}
                  >
                    {prov.enabled && <Icon name="check" size={11} />}
                    {prov.name}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Network Telemetry */}
          {status && (
            <div
              style={{
                marginTop: 8,
                padding: 10,
                background: "var(--well)",
                border: "1px solid var(--line-soft)",
                borderRadius: 6,
                fontSize: 11.5,
                display: "flex",
                flexDirection: "column",
                gap: 5,
              }}
            >
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ color: "var(--faint)", fontWeight: 600 }}>ДИАГНОСТИКА СЕТИ</span>
                <button
                  className="btn sm bordered"
                  onClick={onRemeasureVpn}
                  disabled={busy}
                  style={{ height: 22, fontSize: 10.5, padding: "0 6px" }}
                >
                  <Icon name="refresh-cw" size={11} />
                  Замерить
                </button>
              </div>

              <div style={{ display: "flex", justifyContent: "space-between" }}>
                <span style={{ color: "var(--dim)" }}>Маршрутизация VPN:</span>
                <span className="mono" style={{ color: "var(--text)" }}>
                  {status.vpn ? vpnStatusMap[status.vpn] || status.vpn : "—"}
                </span>
              </div>

              <div style={{ display: "flex", justifyContent: "space-between" }}>
                <span style={{ color: "var(--dim)" }}>Точка выхода (Egress):</span>
                <span className="mono" style={{ color: "var(--text)" }}>
                  {status.relay_egress ? egressMap[status.relay_egress] || status.relay_egress : "Прямое"}
                </span>
              </div>

              <div style={{ display: "flex", justifyContent: "space-between" }}>
                <span style={{ color: "var(--dim)" }}>Служба релея:</span>
                <span className="mono" style={{ color: status.relay_running ? "var(--ok)" : "var(--faint)" }}>
                  {status.relay_running ? "Активна" : "Остановлена"}
                </span>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
