import { useState } from "react";
import EcuInfoSection from "./EcuInfoSection";
import RoutinesPanel from "./RoutinesPanel";
import * as api from "../lib/tauri";
import type { EcuInfoEntry } from "../types";

interface Props {
  connected: boolean;
}

export default function ImcPanel({ connected }: Props) {
  const [ccfEntries, setCcfEntries] = useState<EcuInfoEntry[]>([]);
  const [ccfLoading, setCcfLoading] = useState(false);
  const [ccfError, setCcfError] = useState<string | null>(null);

  const readCcf = async () => {
    setCcfLoading(true);
    setCcfError(null);
    try {
      const data = await api.readCcf();
      setCcfEntries(data);
    } catch (e) {
      setCcfError(String(e));
    } finally {
      setCcfLoading(false);
    }
  };

  return (
    <div className="space-y-6 max-w-2xl">
      <h2 className="text-lg font-bold text-accent">IMC</h2>
      <EcuInfoSection ecuId="imc" connected={connected} />

      {/* CCF Section */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-[#cccccc]">Configuration (CCF)</h3>
          <button
            onClick={readCcf}
            disabled={!connected || ccfLoading}
            className="btn btn-primary text-xs"
          >
            {ccfLoading ? "Reading..." : "Read CCF"}
          </button>
        </div>

        {ccfError && <p className="text-err text-xs">{ccfError}</p>}

        {ccfEntries.length > 0 && (
          <div className="card">
            {ccfEntries.map((entry, i) => (
              <div
                key={`${entry.label}-${i}`}
                className="flex justify-between py-1.5 border-b border-[#444] last:border-0"
              >
                <span className="text-[#858585] text-xs uppercase tracking-wider">
                  {entry.label}
                </span>
                <span className="font-mono text-sm max-w-[60%] text-right break-all">
                  {entry.value ? (
                    <span className="text-[#cccccc]">{entry.value}</span>
                  ) : entry.error ? (
                    <span className="text-err">{entry.error}</span>
                  ) : (
                    <span className="text-[#858585]">&mdash;</span>
                  )}
                </span>
              </div>
            ))}
          </div>
        )}

        {ccfEntries.length === 0 && !ccfLoading && !ccfError && (
          <div className="card text-center text-[#858585] text-sm py-6">
            {connected ? "Press Read CCF to retrieve configuration" : "Connect to read CCF"}
          </div>
        )}
      </div>

      <RoutinesPanel connected={connected} />
    </div>
  );
}
