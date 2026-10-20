import React, { useState } from "react";
import { ReportOutcomeDto } from "../api";
import { Icon } from "../icons";

interface FooterProps {
  pcCode: string;
  busy: boolean;
  onRefresh: () => void;
  onSaveReport: () => Promise<ReportOutcomeDto>;
}

export const Footer: React.FC<FooterProps> = ({
  pcCode,
  busy,
  onRefresh,
  onSaveReport,
}) => {
  const [copiedPc, setCopiedPc] = useState(false);
  const [reportFeedback, setReportFeedback] = useState<string | null>(null);

  const handleCopyPc = () => {
    if (!pcCode) return;
    navigator.clipboard.writeText(pcCode);
    setCopiedPc(true);
    setTimeout(() => setCopiedPc(false), 2000);
  };

  const handleSaveReport = async () => {
    try {
      const outcome = await onSaveReport();
      navigator.clipboard.writeText(outcome.text);
      if (outcome.saved_path) {
        setReportFeedback("Отчёт сохранён на Рабочий стол");
      } else {
        setReportFeedback("Отчёт скопирован в буфер");
      }
      setTimeout(() => setReportFeedback(null), 3500);
    } catch {
      setReportFeedback("Ошибка создания отчёта");
      setTimeout(() => setReportFeedback(null), 3000);
    }
  };

  return (
    <footer className="footer">
      <div className="footer-left">
        {pcCode && (
          <button
            className="btn sm bordered mono"
            onClick={handleCopyPc}
            title="Нажмите, чтобы скопировать аппаратный код ПК"
            style={{ fontSize: 11, padding: "0 8px" }}
          >
            ID: {pcCode}
            {copiedPc ? " (скопирован)" : ""}
          </button>
        )}

        <button
          className="btn sm icon-only bordered"
          onClick={onRefresh}
          disabled={busy}
          title="Глубокое обновление состояния"
        >
          <Icon name="refresh-cw" size={12} />
        </button>

        {reportFeedback && (
          <span style={{ fontSize: 11.5, color: "var(--ok)", fontWeight: 500 }}>
            {reportFeedback}
          </span>
        )}
      </div>

      <div className="footer-right">
        <button
          className="btn sm bordered"
          onClick={handleSaveReport}
          disabled={busy}
          title="Собрать диагностику и сохранить на Рабочий стол"
        >
          <Icon name="copy" size={12} />
          Диагностический отчёт
        </button>
      </div>
    </footer>
  );
};
