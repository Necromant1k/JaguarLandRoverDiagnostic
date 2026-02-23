import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import EcuInfoSection from "./EcuInfoSection";

interface Props {
  connected: boolean;
}

export default function IpcPanel({ connected }: Props) {
  const [scanning, setScanning] = useState(false);
  const [scanResult, setScanResult] = useState<string | null>(null);

  async function handleScanIpc() {
    setScanning(true);
    setScanResult(null);
    try {
      const result = await invoke<string>("scan_ipc_full");
      setScanResult(result);
    } catch (e) {
      setScanResult(`Error: ${e}`);
    } finally {
      setScanning(false);
    }
  }

  return (
    <div className="space-y-6 max-w-2xl">
      <h2 className="text-lg font-bold text-accent">IPC — Instrument Panel Cluster</h2>
      <EcuInfoSection ecuId="ipc" connected={connected} />

      <div className="card space-y-3">
        <h3 className="text-sm font-semibold text-[#cccccc]">Full IPC Scan</h3>
        <p className="text-xs text-[#858585]">
          Reads all IPC DIDs (MDX_IPC X260 EXML) — odometer, service intervals, start auth.
          Saves raw bytes to <code>ipc_dump.json</code> next to the exe.
        </p>
        <button
          className="btn"
          disabled={!connected || scanning}
          onClick={handleScanIpc}
        >
          {scanning ? "Scanning IPC…" : "Scan IPC → ipc_dump.json"}
        </button>
        {scanResult && (
          <p className={`text-xs ${scanResult.startsWith("Error") ? "text-red-400" : "text-green-400"}`}>
            {scanResult}
          </p>
        )}
      </div>
    </div>
  );
}
