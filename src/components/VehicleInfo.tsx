import { useState } from "react";
import * as api from "../lib/tauri";
import type { VehicleInfo as VehicleInfoType } from "../types";

interface Props {
  connected: boolean;
}

export default function VehicleInfo({ connected }: Props) {
  const [info, setInfo] = useState<VehicleInfoType | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleRead = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await api.readVehicleInfo();
      setInfo(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const InfoRow = ({
    label,
    value,
  }: {
    label: string;
    value: string | number | null;
  }) => (
    <div className="flex justify-between py-1.5 border-b border-gray-700/30">
      <span className="text-gray-400 text-xs uppercase tracking-wider">
        {label}
      </span>
      <span className="font-mono text-sm text-gray-200">
        {value ?? <span className="text-gray-600">—</span>}
      </span>
    </div>
  );

  return (
    <div className="space-y-4 max-w-xl">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-bold text-accent">Vehicle Information</h2>
        <button
          onClick={handleRead}
          disabled={!connected || loading}
          className="btn btn-primary"
        >
          {loading ? "Reading..." : "Read Info"}
        </button>
      </div>

      {error && <p className="text-err text-xs">{error}</p>}

      {info && (
        <>
          <div className="card">
            <h3 className="text-sm font-semibold text-gray-300 mb-2">
              Identification
            </h3>
            <InfoRow label="VIN" value={info.vin} />
            <InfoRow
              label="Battery Voltage"
              value={info.voltage !== null ? `${info.voltage.toFixed(1)} V` : null}
            />
            <InfoRow label="Diagnostic Session" value={info.session} />
          </div>

          <div className="card">
            <h3 className="text-sm font-semibold text-gray-300 mb-2">
              IMC Part Numbers
            </h3>
            <InfoRow label="Master RPM (F188)" value={info.master_part} />
            <InfoRow label="v850 (F120)" value={info.v850_part} />
            <InfoRow label="Tuner (F121)" value={info.tuner_part} />
            <InfoRow label="ECU Serial (F18C)" value={info.serial} />
          </div>
        </>
      )}

      {!info && !loading && (
        <div className="card text-center text-gray-500 text-sm py-8">
          Click &quot;Read Info&quot; to query ECU data
        </div>
      )}
    </div>
  );
}
