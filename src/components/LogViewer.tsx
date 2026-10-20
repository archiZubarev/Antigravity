import React, { useEffect, useRef, useState } from "react";
import { LogItemDto } from "../api";
import { Icon } from "../icons";

interface LogViewerProps {
  logs: LogItemDto[];
}

export const LogViewer: React.FC<LogViewerProps> = ({ logs }) => {
  const [isOpen, setIsOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isOpen) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [logs, isOpen]);

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    const text = logs.map((l) => `[${l.level.toUpperCase()}] ${l.text}`).join("\n");
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="card">
      <div
        className="accordion-header"
        onClick={() => setIsOpen(!isOpen)}
        style={{ userSelect: "none" }}
      >
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <Icon name="terminal" size={13} />
          Журнал операций
          <span className="mono" style={{ fontSize: 11, color: "var(--faint)" }}>
            ({logs.length})
          </span>
        </span>

        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {logs.length > 0 && (
            <button
              className="btn sm bordered"
              onClick={handleCopy}
              title="Скопировать журнал"
              style={{ height: 22, fontSize: 10.5, padding: "0 6px" }}
            >
              <Icon name="copy" size={11} />
              {copied ? "Скопировано" : "Копия"}
            </button>
          )}
          <Icon name={isOpen ? "chevron-up" : "chevron-down"} size={14} />
        </div>
      </div>

      {isOpen && (
        <div className="accordion-body">
          <div className="console-well">
            {logs.length === 0 ? (
              <div style={{ color: "var(--faint)", fontStyle: "italic" }}>
                Журнал пуст. Запустите операцию для отображения событий.
              </div>
            ) : (
              logs.map((item, idx) => (
                <div key={idx} className="log-line">
                  <span className={`log-level ${item.level.toLowerCase()}`}>
                    {item.level.toUpperCase()}
                  </span>
                  <span className="log-text">{item.text}</span>
                </div>
              ))
            )}
            <div ref={bottomRef} />
          </div>
        </div>
      )}
    </div>
  );
};
