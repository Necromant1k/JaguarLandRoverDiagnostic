import { useState, useEffect } from "react";
import * as api from "../lib/tauri";
import type { DeviceInfo, J2534DeviceEntry } from "../types";

interface Props {
  connected: boolean;
  deviceInfo: DeviceInfo | null;
  onConnected: (info: DeviceInfo) => void;
  onDisconnected: () => void;
}

const ECU_OPTIONS = [
  { id: "bcm", label: "BCM", address: "0x726" },
  { id: "gwm", label: "GWM", address: "0x716" },
  { id: "ipc", label: "IPC", address: "0x720" },
] as const;

const AUTO_DETECT = "__auto__";
const MANUAL_PATH = "__manual__";

export default function ConnectPanel({
  connected,
  deviceInfo,
  onConnected,
  onDisconnected,
}: Props) {
  const [devices, setDevices] = useState<J2534DeviceEntry[]>([]);
  const [selectedDevice, setSelectedDevice] = useState(AUTO_DETECT);
  const [manualPath, setManualPath] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [benchMode, setBenchMode] = useState(false);
  const [selectedEcus, setSelectedEcus] = useState<Set<string>>(
    new Set(["bcm"])
  );

  useEffect(() => {
    api.discoverDevices().then((d) => {
      setDevices(d);
      // If devices found, default to first device instead of auto
      if (d.length > 0) {
        setSelectedDevice(d[0].dll_path);
      }
    }).catch(() => {});
  }, []);

  const getDllPath = (): string | undefined => {
    if (selectedDevice === AUTO_DETECT) return undefined;
    if (selectedDevice === MANUAL_PATH) return manualPath || undefined;
    return selectedDevice; // dll_path of a discovered device
  };

  const handleConnect = async () => {
    setLoading(true);
    setError(null);
    try {
      const info = await api.connect(getDllPath());
      onConnected(info);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleDisconnect = async () => {
    setLoading(true);
    setError(null);
    try {
      await api.disconnect();
      setBenchMode(false);
      onDisconnected();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleBenchToggle = async () => {
    const newValue = !benchMode;
    try {
      const ecus = Array.from(selectedEcus);
      await api.toggleBenchMode(newValue, ecus);
      setBenchMode(newValue);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleEcuToggle = (ecuId: string) => {
    setSelectedEcus((prev) => {
      const next = new Set(prev);
      if (next.has(ecuId)) {
        if (next.size > 1) {
          next.delete(ecuId);
        }
      } else {
        next.add(ecuId);
      }
      return next;
    });
  };

  const canConnect =
    selectedDevice === AUTO_DETECT ||
    (selectedDevice === MANUAL_PATH && manualPath.trim().length > 0) ||
    (selectedDevice !== MANUAL_PATH && selectedDevice !== AUTO_DETECT);

  return (
    <div className="space-y-4 max-w-xl">
      <h2 className="text-lg font-bold text-accent">J2534 Connection</h2>

      {/* Device Selection */}
      <div className="card space-y-3">
        <label className="block text-xs text-gray-400 uppercase tracking-wider">
          J2534 Device
        </label>
        <select
          value={selectedDevice}
          onChange={(e) => setSelectedDevice(e.target.value)}
          disabled={connected}
          className="w-full bg-bg-primary border border-gray-600 rounded px-3 py-2 text-sm
                     focus:border-accent focus:outline-none disabled:opacity-50"
        >
          <option value={AUTO_DETECT}>Auto-detect (first available)</option>
          {devices.map((d) => (
            <option key={d.dll_path} value={d.dll_path}>
              {d.name}
            </option>
          ))}
          <option value={MANUAL_PATH}>Custom DLL path...</option>
        </select>

        {selectedDevice === MANUAL_PATH && !connected && (
          <input
            type="text"
            value={manualPath}
            onChange={(e) => setManualPath(e.target.value)}
            className="w-full bg-bg-primary border border-gray-600 rounded px-3 py-2 text-sm font-mono
                       focus:border-accent focus:outline-none"
            placeholder="C:\Program Files (x86)\...\device.dll"
          />
        )}

        <div className="flex gap-2">
          {!connected ? (
            <button
              onClick={handleConnect}
              disabled={loading || !canConnect}
              className="btn btn-primary"
            >
              {loading ? "Connecting..." : "Connect"}
            </button>
          ) : (
            <button
              onClick={handleDisconnect}
              disabled={loading}
              className="btn btn-danger"
            >
              {loading ? "Disconnecting..." : "Disconnect"}
            </button>
          )}
        </div>

        {error && (
          <p className="text-err text-xs mt-2">{error}</p>
        )}
      </div>

      {/* Device Info */}
      {connected && deviceInfo && (
        <div className="card space-y-2">
          <h3 className="text-sm font-semibold text-gray-300">Device Info</h3>
          <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
            <span className="text-gray-500">Device</span>
            <span className="font-mono">{deviceInfo.dll_path.split("\\").pop()}</span>
            <span className="text-gray-500">Firmware</span>
            <span className="font-mono">{deviceInfo.firmware_version}</span>
            <span className="text-gray-500">DLL Version</span>
            <span className="font-mono">{deviceInfo.dll_version}</span>
            <span className="text-gray-500">API Version</span>
            <span className="font-mono">{deviceInfo.api_version}</span>
          </div>
        </div>
      )}

      {/* Bench Mode */}
      {connected && (
        <div className="card space-y-3">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-sm font-semibold text-gray-300">
                Bench Mode (ECU Emulation)
              </h3>
              <p className="text-xs text-gray-500 mt-0.5">
                Emulate ECUs on CAN bus for bench testing without vehicle
              </p>
            </div>
            <button
              onClick={handleBenchToggle}
              className={`relative w-10 h-5 rounded-full transition-colors ${
                benchMode ? "bg-accent" : "bg-gray-600"
              }`}
            >
              <span
                className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
                  benchMode ? "translate-x-5" : ""
                }`}
              />
            </button>
          </div>

          {/* Per-ECU checkboxes */}
          <div className="space-y-1.5 pl-1">
            {ECU_OPTIONS.map((ecu) => (
              <label
                key={ecu.id}
                className="flex items-center gap-2 text-xs cursor-pointer"
              >
                <input
                  type="checkbox"
                  checked={selectedEcus.has(ecu.id)}
                  onChange={() => handleEcuToggle(ecu.id)}
                  disabled={benchMode}
                  className="accent-accent"
                />
                <span className="text-gray-300">{ecu.label}</span>
                <span className="text-gray-600 font-mono">({ecu.address})</span>
              </label>
            ))}
          </div>
        </div>
      )}

      {/* Status */}
      <div className="card">
        <div className="flex items-center gap-3">
          <span
            className={`status-led ${connected ? "connected" : "disconnected"}`}
          />
          <span className="text-sm">
            {connected ? (
              <span className="text-ok">Connected to Mongoose Pro</span>
            ) : (
              <span className="text-gray-400">
                Not connected — select device and click Connect
              </span>
            )}
          </span>
        </div>
      </div>
    </div>
  );
}
