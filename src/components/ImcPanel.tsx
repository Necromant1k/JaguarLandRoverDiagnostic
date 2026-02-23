import { useState } from "react";
import EcuInfoSection from "./EcuInfoSection";
import RoutinesPanel from "./RoutinesPanel";
import * as api from "../lib/tauri";
import type { EcuInfoEntry, CcfCompareEntry } from "../types";

interface Props {
  connected: boolean;
}

export default function ImcPanel({ connected }: Props) {
  const [ccfEntries, setCcfEntries] = useState<EcuInfoEntry[]>([]);
  const [ccfLoading, setCcfLoading] = useState(false);
  const [ccfError, setCcfError] = useState<string | null>(null);

  const [compareEntries, setCompareEntries] = useState<CcfCompareEntry[]>([]);
  const [compareLoading, setCompareLoading] = useState(false);
  const [compareError, setCompareError] = useState<string | null>(null);
  const [showMismatchOnly, setShowMismatchOnly] = useState(false);

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

  const compareCcf = async () => {
    setCompareLoading(true);
    setCompareError(null);
    try {
      const data = await api.compareCcf();
      setCompareEntries(data);
    } catch (e) {
      setCompareError(String(e));
    } finally {
      setCompareLoading(false);
    }
  };

  const visibleCompare = showMismatchOnly
    ? compareEntries.filter((e) => e.mismatch)
    : compareEntries;

  const mismatchCount = compareEntries.filter((e) => e.mismatch).length;

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

      {/* CCF Compare Section */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h3 className="text-sm font-semibold text-[#cccccc]">CCF Compare (GWM / BCM / IMC)</h3>
            {mismatchCount > 0 && (
              <span className="text-xs text-err font-mono">{mismatchCount} mismatches</span>
            )}
          </div>
          <div className="flex items-center gap-2">
            {compareEntries.length > 0 && (
              <button
                onClick={() => setShowMismatchOnly((v) => !v)}
                className="btn text-xs"
              >
                {showMismatchOnly ? "Show All" : "Mismatches Only"}
              </button>
            )}
            <button
              onClick={compareCcf}
              disabled={!connected || compareLoading}
              className="btn btn-primary text-xs"
            >
              {compareLoading ? "Reading..." : "Compare CCF"}
            </button>
          </div>
        </div>

        {compareError && <p className="text-err text-xs">{compareError}</p>}

        {visibleCompare.length > 0 && (
          <div className="card overflow-x-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b border-[#444]">
                  <th className="text-left text-[#858585] py-1.5 pr-3 font-normal w-6">ID</th>
                  <th className="text-left text-[#858585] py-1.5 pr-3 font-normal">Option</th>
                  <th className="text-left text-[#858585] py-1.5 pr-3 font-normal">GWM</th>
                  <th className="text-left text-[#858585] py-1.5 pr-3 font-normal">BCM</th>
                  <th className="text-left text-[#858585] py-1.5 font-normal">IMC</th>
                </tr>
              </thead>
              <tbody>
                {visibleCompare.map((entry) => (
                  <tr
                    key={entry.option_id}
                    className={`border-b border-[#333] last:border-0 ${entry.mismatch ? "bg-[#3a1a1a]" : ""}`}
                  >
                    <td className="py-1.5 pr-3 text-[#555] font-mono">{entry.option_id}</td>
                    <td className="py-1.5 pr-3 text-[#aaaaaa]">{entry.name}</td>
                    <td className="py-1.5 pr-3 font-mono text-[#cccccc]">
                      {entry.gwm ?? <span className="text-[#555]">—</span>}
                    </td>
                    <td className="py-1.5 pr-3 font-mono text-[#cccccc]">
                      {entry.bcm ?? <span className="text-[#555]">—</span>}
                    </td>
                    <td className={`py-1.5 font-mono ${entry.mismatch ? "text-err" : "text-[#cccccc]"}`}>
                      {entry.imc ?? <span className="text-[#555]">—</span>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {compareEntries.length === 0 && !compareLoading && !compareError && (
          <div className="card text-center text-[#858585] text-sm py-6">
            {connected
              ? "Press Compare CCF to read GWM/BCM/IMC and find mismatches"
              : "Connect to compare CCF"}
          </div>
        )}
      </div>

      <RoutinesPanel connected={connected} />
    </div>
  );
}
