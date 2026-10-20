import React from "react";
import { Icon, Spinner } from "../icons";

interface HeaderProps {
  version: string;
  busy: string | null;
}

export const Header: React.FC<HeaderProps> = ({ version, busy }) => {
  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <div className="brand-icon">
          <Icon name="unlock-beam" size={14} />
        </div>
        <span className="brand-title">ANTIGRAVITY UNLOCKER</span>
        {version && <span className="brand-badge">v{version}</span>}
      </div>

      <div className="titlebar-actions">
        {busy ? (
          <div className="busy-indicator">
            <Spinner size={13} />
            <span>{busy}</span>
          </div>
        ) : (
          <div className="busy-indicator" style={{ color: "var(--faint)" }}>
            <span>Система готова</span>
          </div>
        )}
      </div>
    </header>
  );
};
