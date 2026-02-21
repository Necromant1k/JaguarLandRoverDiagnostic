import { useState } from "react";
import * as api from "../lib/tauri";
import type { SshResult } from "../types";

interface Props {
  connected: boolean;
}

export default function SshPanel({ connected }: Props) {
  const [result, setResult] = useState<SshResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleEnableSsh = async () => {
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      const res = await api.enableSsh();
      setResult(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-4 max-w-xl">
      <h2 className="text-lg font-bold text-accent">SSH Access</h2>

      <div className="card text-center space-y-4 py-8">
        <p className="text-gray-400 text-sm">
          Enable SSH on the IMC for remote diagnostics.
          <br />
          This performs: TesterPresent &rarr; ExtendedSession &rarr; SecurityAccess
          0x11 &rarr; Routine 0x603E
        </p>

        <button
          onClick={handleEnableSsh}
          disabled={!connected || loading}
          className="btn btn-success text-lg px-8 py-3"
        >
          {loading ? (
            <span className="flex items-center gap-2">
              <span className="status-led pending" />
              Enabling SSH...
            </span>
          ) : (
            "Enable SSH"
          )}
        </button>

        {error && (
          <div className="bg-err/10 border border-err/30 rounded p-3 text-err text-sm">
            {error}
          </div>
        )}

        {result && (
          <div
            className={`border rounded p-4 ${
              result.success
                ? "bg-ok/10 border-ok/30"
                : "bg-err/10 border-err/30"
            }`}
          >
            {result.success ? (
              <>
                <p className="text-ok font-bold text-lg mb-2">SSH ENABLED</p>
                <p className="text-gray-300 font-mono text-sm">
                  Connect: <span className="text-accent">root@{result.ip_address}</span>
                </p>
              </>
            ) : (
              <p className="text-err">{result.message}</p>
            )}
          </div>
        )}
      </div>

      <div className="card text-xs text-gray-500 space-y-1">
        <p>
          <span className="text-gray-400">Protocol:</span> UDS over ISO-15765
          (CAN) via J2534
        </p>
        <p>
          <span className="text-gray-400">ECU:</span> IMC (TX: 0x7B3, RX: 0x7BB)
        </p>
        <p>
          <span className="text-gray-400">Security:</span> Level 0x11, KeyGenMkI
          (DC0314)
        </p>
        <p>
          <span className="text-gray-400">Routine:</span> 0x603E — Engineering
          Screen Level 3 (SSH)
        </p>
      </div>
    </div>
  );
}
