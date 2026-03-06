import { useState } from "react";
import EcuInfoSection from "./EcuInfoSection";
import RoutinesPanel from "./RoutinesPanel";
import * as api from "../lib/tauri";
import type { EcuInfoEntry, CcfCompareEntry, CanSniffResult } from "../types";

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

  const [sniffResult, setSniffResult] = useState<CanSniffResult | null>(null);
  const [sniffLoading, setSniffLoading] = useState(false);
  const [sniffError, setSniffError] = useState<string | null>(null);

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

  const runCanSniff = async () => {
    setSniffLoading(true);
    setSniffError(null);
    try {
      const data = await api.canSniffRoutine();
      setSniffResult(data);
    } catch (e) {
      setSniffError(String(e));
    } finally {
      setSniffLoading(false);
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
                    <td className={`py-1.5 pr-3 font-mono ${entry.mismatch ? "text-err" : "text-[#cccccc]"}`}>
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
              ? "Press Compare CCF to read GWM/BCM and find mismatches (IMC = GWM source)"
              : "Connect to compare CCF"}
          </div>
        )}
      </div>

      {/* CAN Sniff during 0x6038 */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-[#cccccc]">CAN Sniff (0x6038 debug)</h3>
          <button
            onClick={runCanSniff}
            disabled={!connected || sniffLoading}
            className="btn btn-primary text-xs"
          >
            {sniffLoading ? "Sniffing (~35s)..." : "Sniff 0x6038"}
          </button>
        </div>

        {sniffError && <p className="text-err text-xs">{sniffError}</p>}

        {sniffResult && (
          <div className="card space-y-2">
            <p className="text-xs text-[#aaaaaa]">{sniffResult.summary}</p>
            {sniffResult.routine_response && (
              <p className="text-xs font-mono text-accent">
                Response: {sniffResult.routine_response}
              </p>
            )}
            {sniffResult.new_can_ids.length > 0 ? (
              <div>
                <p className="text-xs text-[#cccccc] font-semibold">New CAN IDs after 0x6038:</p>
                <p className="text-xs font-mono text-accent">
                  {sniffResult.new_can_ids.join(", ")}
                </p>
              </div>
            ) : (
              <p className="text-xs text-[#858585]">
                No new CAN IDs — IMC does not send CAN requests during 0x6038
              </p>
            )}
            <details className="text-xs">
              <summary className="text-[#858585] cursor-pointer">
                Baseline: {sniffResult.baseline_frames.length} frames |
                After: {sniffResult.after_frames.length} frames
              </summary>
              <div className="mt-2 max-h-40 overflow-y-auto">
                <p className="text-[#858585] font-semibold mb-1">After 0x6038 (first 100):</p>
                {sniffResult.after_frames.slice(0, 100).map((f, i) => (
                  <div key={i} className="font-mono text-[#aaaaaa]">
                    {f.timestamp_ms}ms {f.can_id} [{f.data_len}] {f.data_hex}
                  </div>
                ))}
              </div>
            </details>
          </div>
        )}

        {!sniffResult && !sniffLoading && !sniffError && (
          <div className="card text-center text-[#858585] text-sm py-6">
            {connected
              ? "Sends 0x6038, then captures all CAN traffic to see what IMC does"
              : "Connect to sniff CAN"}
          </div>
        )}
      </div>

      <RoutinesPanel connected={connected} />
    </div>
  );
}
