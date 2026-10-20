import React from "react";
import { HeadlineDto } from "../api";
import { Icon, Spinner } from "../icons";

interface StatusHeroProps {
  headline?: HeadlineDto;
  busy: string | null;
  onEnableAll: () => void;
  onRepair: () => void;
  onOpenSettings: () => void;
  onRestartClient: () => void;
}

export const StatusHero: React.FC<StatusHeroProps> = ({
  headline,
  busy,
  onEnableAll,
  onRepair,
  onOpenSettings,
  onRestartClient,
}) => {
  const tone = headline?.tone || "off";
  const title = headline?.title || "Инициализация состояния...";
  const detail = headline?.detail || "Опрашиваем установленные клиенты и системные службы...";
  const action = headline?.action;

  const isBusy = Boolean(busy);

  return (
    <div className="hero-card">
      <div className="hero-status-line">
        <span className={`dot ${tone}`} />
        <h2 className="hero-title">{title}</h2>
      </div>

      <p className="hero-detail">{detail}</p>

      <div className="hero-actions">
        {action === "enable_all" && (
          <button className="btn primary" onClick={onEnableAll} disabled={isBusy}>
            {isBusy ? <Spinner size={13} /> : <Icon name="check" size={14} />}
            Включить всё
          </button>
        )}

        {action === "repair" && (
          <button className="btn primary" onClick={onRepair} disabled={isBusy}>
            {isBusy ? <Spinner size={13} /> : <Icon name="tool" size={14} />}
            Починить
          </button>
        )}

        {action === "elevate" && (
          <button className="btn primary" onClick={onOpenSettings} disabled={isBusy}>
            <Icon name="shield" size={14} />
            Открыть настройки доступа
          </button>
        )}

        {action === "verify" && (
          <button className="btn primary" onClick={onRepair} disabled={isBusy}>
            <Icon name="refresh-cw" size={14} />
            Проверить
          </button>
        )}

        {!action && tone === "ok" && (
          <button className="btn bordered" onClick={onRestartClient} disabled={isBusy} title="Перезапустить приложение Antigravity">
            <Icon name="refresh-cw" size={13} />
            Перезапустить клиент
          </button>
        )}
      </div>
    </div>
  );
};
