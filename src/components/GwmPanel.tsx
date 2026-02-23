import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import EcuInfoSection from "./EcuInfoSection";

interface Props {
  connected: boolean;
}

export default function GwmPanel({ connected }: Props) {
  const [scanning, setScanning] = useState(false);
  const [scanResult, setScanResult] = useState<string | null>(null);

  async function handleScanGwm() {
    setScanning(true);
    setScanResult(null);
    try {
      const result = await invoke<string>("scan_gwm_full");
      setScanResult(result);
    } catch (e) {
      setScanResult(`Error: ${e}`);
    } finally {
      setScanning(false);
    }
  }

  return (
    <div className="space-y-6 max-w-2xl">
      <h2 className="text-lg font-bold text-accent">GWM — Gateway / Battery Manager</h2>
      <EcuInfoSection ecuId="gwm" connected={connected} />

      <div className="card space-y-3">
        <h3 className="text-sm font-semibold text-[#cccccc]">Full GWM Scan</h3>
        <p className="text-xs text-[#858585]">
          Reads all GWM DIDs (MDX_GWM X260 EXML) — battery voltage, SoC, temp, charge stats.
          Saves raw bytes to <code>gwm_dump.json</code> next to the exe.
        </p>
        <button
          className="btn"
          disabled={!connected || scanning}
          onClick={handleScanGwm}
        >
          {scanning ? "Scanning GWM…" : "Scan GWM → gwm_dump.json"}
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
