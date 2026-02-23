import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import EcuInfoSection from "./EcuInfoSection";

interface Props {
  connected: boolean;
}

export default function BcmPanel({ connected }: Props) {
  const [scanning, setScanning] = useState(false);
  const [scanResult, setScanResult] = useState<string | null>(null);

  async function handleScanBcm() {
    setScanning(true);
    setScanResult(null);
    try {
      const result = await invoke<string>("scan_bcm_full");
      setScanResult(result);
    } catch (e) {
      setScanResult(`Error: ${e}`);
    } finally {
      setScanning(false);
    }
  }

  return (
    <div className="space-y-6 max-w-2xl">
      <h2 className="text-lg font-bold text-accent">BCM</h2>
      <EcuInfoSection ecuId="bcm" connected={connected} />

      <div className="card space-y-3">
        <h3 className="text-sm font-semibold text-[#cccccc]">Full BCM Scan</h3>
        <p className="text-xs text-[#858585]">
          Reads all 60+ BCM DIDs (MDX_BCM X260 EXML) in default + extended session.
          Saves raw bytes to <code>bcm_dump.json</code> for bench emulation.
        </p>
        <button
          className="btn"
          disabled={!connected || scanning}
          onClick={handleScanBcm}
        >
          {scanning ? "Scanning BCM…" : "Scan BCM → bcm_dump.json"}
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
