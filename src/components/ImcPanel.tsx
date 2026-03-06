import { useState } from "react";
import EcuInfoSection from "./EcuInfoSection";
import RoutinesPanel from "./RoutinesPanel";
import * as api from "../lib/tauri";
import type {
  EcuInfoEntry,
  CcfCompareEntry,
  CanSniffResult,
  RestoreCcfResult,
} from "../types";

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

  const [restoreResult, setRestoreResult] = useState<RestoreCcfResult | null>(null);
  const [restoreLoading, setRestoreLoading] = useState(false);
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [restoreConfirm, setRestoreConfirm] = useState(false);
  const [restoreSniff, setRestoreSniff] = useState(false);
  const [restoreStep, setRestoreStep] = useState<string | null>(null);

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

  const runRestoreCcf = async () => {
    setRestoreLoading(true);
    setRestoreError(null);
    setRestoreResult(null);
    setRestoreConfirm(false);
    setRestoreStep("Pre-flight: Reading GWM CCF...");
    try {
      const data = await api.restoreCcf(restoreSniff);
      setRestoreResult(data);
      setRestoreStep(null);
    } catch (e) {
      setRestoreError(String(e));
      setRestoreStep(null);
    } finally {
      setRestoreLoading(false);
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

      {/* Restore CCF Section */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-[#cccccc]">Restore CCF (SDD Sequence)</h3>
          <div className="flex items-center gap-2">
            <label className="flex items-center gap-1.5 text-xs text-[#858585] cursor-pointer">
              <input
                type="checkbox"
                checked={restoreSniff}
                onChange={(e) => setRestoreSniff(e.target.checked)}
                disabled={restoreLoading}
                className="accent-accent"
              />
              CAN Sniff
            </label>
            {!restoreConfirm ? (
              <button
                onClick={() => setRestoreConfirm(true)}
                disabled={!connected || restoreLoading}
                className="btn text-xs bg-[#8b2020] hover:bg-[#a62828] text-white border-[#a62828]"
              >
                Restore CCF
              </button>
            ) : (
              <div className="flex items-center gap-1">
                <span className="text-xs text-err">IMC will reboot (~2 min). Continue?</span>
                <button
                  onClick={runRestoreCcf}
                  className="btn text-xs bg-[#8b2020] hover:bg-[#a62828] text-white border-[#a62828]"
                >
                  Confirm
                </button>
                <button
                  onClick={() => setRestoreConfirm(false)}
                  className="btn text-xs"
                >
                  Cancel
                </button>
              </div>
            )}
          </div>
        </div>

        {restoreLoading && restoreStep && (
          <div className="card text-center text-accent text-sm py-4 animate-pulse">
            {restoreStep}
          </div>
        )}

        {restoreError && <p className="text-err text-xs">{restoreError}</p>}

        {restoreResult && (
          <div className="space-y-3">
            {/* Overall status */}
            <div className={`card py-2 px-3 text-sm font-semibold ${restoreResult.success ? "text-green-400 border-green-800" : "text-err border-red-800"}`}>
              {restoreResult.success ? "RESTORE COMPLETE — ALL STEPS PASSED" : "RESTORE FAILED"}
            </div>

            {/* Pre-flight */}
            {restoreResult.pre_flight && (
              <div className="card space-y-2">
                <p className="text-xs font-semibold text-[#cccccc]">Pre-Flight: GWM CCF</p>
                <div className="text-xs space-y-1">
                  <div className="flex justify-between">
                    <span className="text-[#858585]">Option 467 (Display)</span>
                    <span className="font-mono text-[#cccccc]">
                      {restoreResult.pre_flight.option_467_extracted != null
                        ? `0x${restoreResult.pre_flight.option_467_extracted.toString(16).toUpperCase().padStart(2, "0")} — ${restoreResult.pre_flight.option_467_desc}`
                        : "Not available"}
                    </span>
                  </div>
                  {restoreResult.pre_flight.option_467_raw != null && (
                    <div className="flex justify-between">
                      <span className="text-[#858585]">Raw byte</span>
                      <span className="font-mono text-[#777]">
                        0x{restoreResult.pre_flight.option_467_raw.toString(16).toUpperCase().padStart(2, "0")}
                      </span>
                    </div>
                  )}
                  {restoreResult.pre_flight.warnings.map((w, i) => (
                    <p key={i} className="text-err text-xs">{w}</p>
                  ))}
                </div>
              </div>
            )}

            {/* Steps table */}
            <div className="card overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b border-[#444]">
                    <th className="text-left text-[#858585] py-1.5 pr-3 font-normal">Step</th>
                    <th className="text-left text-[#858585] py-1.5 pr-3 font-normal w-10">Status</th>
                    <th className="text-left text-[#858585] py-1.5 pr-3 font-normal">Detail</th>
                    <th className="text-right text-[#858585] py-1.5 font-normal w-16">Time</th>
                  </tr>
                </thead>
                <tbody>
                  {restoreResult.steps.map((step, i) => (
                    <tr key={i} className={`border-b border-[#333] last:border-0 ${!step.success ? "bg-[#3a1a1a]" : ""}`}>
                      <td className="py-1.5 pr-3 text-[#cccccc]">{step.name}</td>
                      <td className={`py-1.5 pr-3 font-mono ${step.success ? "text-green-400" : "text-err"}`}>
                        {step.success ? "OK" : "FAIL"}
                      </td>
                      <td className="py-1.5 pr-3 font-mono text-[#aaaaaa] max-w-[300px] truncate" title={step.detail}>
                        {step.detail}
                      </td>
                      <td className="py-1.5 text-right font-mono text-[#777]">
                        {step.duration_ms < 1000
                          ? `${step.duration_ms}ms`
                          : `${(step.duration_ms / 1000).toFixed(1)}s`}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* CAN sniff frames */}
            {restoreResult.sniff_frames.length > 0 && (
              <details className="card">
                <summary className="text-xs text-[#858585] cursor-pointer py-1">
                  CAN Sniff: {restoreResult.sniff_frames.length} frames during 0x0E06
                </summary>
                <div className="mt-2 max-h-48 overflow-y-auto">
                  {restoreResult.sniff_frames.slice(0, 200).map((f, i) => (
                    <div key={i} className="font-mono text-xs text-[#aaaaaa]">
                      {f.timestamp_ms}ms {f.can_id} [{f.data_len}] {f.data_hex}
                    </div>
                  ))}
                  {restoreResult.sniff_frames.length > 200 && (
                    <p className="text-[#555] text-xs mt-1">
                      ...and {restoreResult.sniff_frames.length - 200} more
                    </p>
                  )}
                </div>
              </details>
            )}

            {/* Post-flight */}
            {restoreResult.post_flight && (
              <div className="card space-y-2">
                <div className="flex items-center justify-between">
                  <p className="text-xs font-semibold text-[#cccccc]">Post-Flight: IMC after reboot</p>
                  <span className={`text-xs font-mono ${restoreResult.post_flight.imc_responsive ? "text-green-400" : "text-err"}`}>
                    {restoreResult.post_flight.imc_responsive ? "RESPONSIVE" : "NO RESPONSE"}
                  </span>
                </div>
                {restoreResult.post_flight.dids_read.map((entry, i) => (
                  <div
                    key={i}
                    className="flex justify-between py-1 border-b border-[#333] last:border-0 text-xs"
                  >
                    <span className="text-[#858585]">{entry.label} ({entry.did_hex})</span>
                    <span className="font-mono max-w-[55%] text-right break-all">
                      {entry.value ? (
                        <span className="text-[#cccccc]">{entry.value}</span>
                      ) : entry.error ? (
                        <span className="text-err">{entry.error}</span>
                      ) : (
                        <span className="text-[#555]">—</span>
                      )}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {!restoreResult && !restoreLoading && !restoreError && (
          <div className="card text-center text-[#858585] text-sm py-6">
            {connected
              ? "Runs 0x0E08 → 0x0E06 → 0x6038 → ECU Reset. Reads GWM CCF first, verifies IMC after reboot."
              : "Connect to restore CCF"}
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
