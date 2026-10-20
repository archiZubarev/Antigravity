import React, { useState } from "react";
import { InstallDto } from "../api";
import { Icon } from "../icons";

interface InstallationsProps {
  installs: InstallDto[];
  busy: boolean;
  onAddPath: (path: string) => Promise<void>;
  onForgetPath: (path: string) => void;
}

export const Installations: React.FC<InstallationsProps> = ({
  installs,
  busy,
  onAddPath,
  onForgetPath,
}) => {
  const [isAdding, setIsAdding] = useState(false);
  const [customPath, setCustomPath] = useState("");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!customPath.trim()) return;
    try {
      setErrorMsg(null);
      await onAddPath(customPath.trim());
      setCustomPath("");
      setIsAdding(false);
    } catch (err: any) {
      setErrorMsg(typeof err === "string" ? err : "Не удалось добавить путь");
    }
  };

  return (
    <div className="card">
      <div className="card-header">
        <span className="card-title">Обнаруженные клиенты</span>
        {!isAdding && (
          <button
            className="btn sm bordered"
            onClick={() => setIsAdding(true)}
            disabled={busy}
            title="Указать путь к Antigravity вручную"
          >
            <Icon name="plus" size={13} />
            Добавить путь
          </button>
        )}
      </div>

      {isAdding && (
        <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ display: "flex", gap: 8 }}>
            <input
              type="text"
              className="input-box"
              placeholder="/Applications/Antigravity.app"
              value={customPath}
              onChange={(e) => setCustomPath(e.target.value)}
              autoFocus
            />
            <button type="submit" className="btn sm primary" disabled={busy || !customPath.trim()}>
              ОК
            </button>
            <button
              type="button"
              className="btn sm bordered"
              onClick={() => {
                setIsAdding(false);
                setErrorMsg(null);
              }}
            >
              Отмена
            </button>
          </div>
          {errorMsg && (
            <span style={{ fontSize: 11, color: "var(--danger)", paddingLeft: 2 }}>{errorMsg}</span>
          )}
        </form>
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {installs.length === 0 ? (
          <div style={{ fontSize: 12, color: "var(--faint)", padding: "4px 0" }}>
            Клиенты Antigravity не обнаружены в стандартных директориях.
          </div>
        ) : (
          installs.map((inst, index) => (
            <div key={inst.path || index} className="install-item">
              <div className="install-main">
                <span className="install-label">{inst.label}</span>
                {inst.path && <span className="install-path mono">{inst.path}</span>}
              </div>

              <div style={{ display: "flex", alignItems: "center", gap: 8, flexShrink: 0 }}>
                {inst.patched === true && <span className="chip ok">Пропатчен</span>}
                {inst.patched === false && <span className="chip warn">Не пропатчен</span>}
                {inst.patched === undefined && <span className="chip">Статус неизвестен</span>}

                {inst.manual && inst.path && (
                  <button
                    className="btn sm icon-only bordered"
                    onClick={() => onForgetPath(inst.path!)}
                    title="Забыть добавленный вручную путь"
                    disabled={busy}
                  >
                    <Icon name="trash" size={12} color="var(--faint)" />
                  </button>
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
