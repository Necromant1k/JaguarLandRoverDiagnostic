import { useState, useEffect, useCallback } from "react";
import * as api from "../lib/tauri";
import type { EcuInfoEntry } from "../types";

interface Props {
  ecuId: "imc" | "bcm";
  connected: boolean;
}

export default function EcuInfoSection({ ecuId, connected }: Props) {
  const [entries, setEntries] = useState<EcuInfoEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchInfo = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await api.readEcuInfo(ecuId);
      setEntries(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [ecuId]);

  useEffect(() => {
    if (connected) {
      fetchInfo();
    }
  }, [connected, fetchInfo]);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-[#cccccc]">ECU Info</h3>
        <button
          onClick={fetchInfo}
          disabled={!connected || loading}
          className="btn btn-primary text-xs"
        >
          {loading ? "Reading..." : "Refresh"}
        </button>
      </div>

      {error && <p className="text-err text-xs">{error}</p>}

      {entries.length > 0 && (
        <div className="card">
          {entries.map((entry) => (
            <div
              key={entry.did_hex}
              className="flex justify-between py-1.5 border-b border-[#444] last:border-0"
            >
              <span className="text-[#858585] text-xs uppercase tracking-wider">
                {entry.label}{" "}
                <span className="text-[#858585] font-mono">({entry.did_hex})</span>
              </span>
              <span className="font-mono text-sm">
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

      {entries.length === 0 && !loading && !error && (
        <div className="card text-center text-[#858585] text-sm py-8">
          {connected ? "No data yet" : "Connect to read ECU info"}
        </div>
      )}

      {loading && entries.length === 0 && (
        <div className="card text-center text-[#858585] text-sm py-8">
          Reading ECU info...
        </div>
      )}
    </div>
  );
}
