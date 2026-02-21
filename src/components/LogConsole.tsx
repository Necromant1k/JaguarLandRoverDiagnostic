import { useEffect, useRef, useState } from "react";
import * as api from "../lib/tauri";
import type { LogEntry } from "../types";

interface Props {
  logs: LogEntry[];
  onClear: () => void;
}

const directionColors: Record<string, string> = {
  Tx: "text-tx",
  Rx: "text-rx",
  Error: "text-err",
  Pending: "text-pending",
};

const directionLabels: Record<string, string> = {
  Tx: "TX",
  Rx: "RX",
  Error: "ERR",
  Pending: "...",
};

function formatLogText(logs: LogEntry[]): string {
  return logs
    .map(
      (e) =>
        `${e.timestamp} [${e.direction.padEnd(3)}] ${e.data_hex}${e.description ? " " + e.description : ""}`
    )
    .join("\n");
}

export default function LogConsole({ logs, onClear }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  const handleCopyAll = async () => {
    try {
      let text = "";
      try {
        text = await api.exportLogs();
      } catch {
        text = "UDS App Log\n---\n";
      }
      text += formatLogText(logs);
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // fallback
    }
  };

  return (
    <div className="h-48 bg-bg-secondary border-t border-gray-700/50 flex flex-col shrink-0">
      <div className="flex items-center justify-between px-3 py-1 border-b border-gray-700/30">
        <span className="text-xs text-gray-400 uppercase tracking-wider">
          UDS Log
        </span>
        <div className="flex items-center gap-2">
          <span className="text-xs text-gray-600">{logs.length} entries</span>
          <button
            onClick={handleCopyAll}
            className="text-xs text-gray-500 hover:text-gray-300"
          >
            {copied ? "Copied!" : "Copy All"}
          </button>
          <button
            onClick={onClear}
            className="text-xs text-gray-500 hover:text-gray-300"
          >
            Clear
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-auto px-3 py-1 font-mono text-xs leading-5">
        {logs.map((entry, i) => (
          <div key={i} className="flex gap-2">
            <span className="text-gray-600 w-20 shrink-0">{entry.timestamp}</span>
            <span
              className={`w-6 shrink-0 font-bold ${
                directionColors[entry.direction] ?? "text-gray-400"
              }`}
            >
              {directionLabels[entry.direction] ?? "???"}
            </span>
            <span
              className={directionColors[entry.direction] ?? "text-gray-400"}
            >
              {entry.data_hex}
            </span>
            {entry.description && (
              <span className="text-gray-500 ml-2">{entry.description}</span>
            )}
          </div>
        ))}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
