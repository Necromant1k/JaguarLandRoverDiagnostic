import { useState, useEffect } from "react";
import * as api from "../lib/tauri";
import type { DeviceInfo, J2534DeviceEntry } from "../types";

interface Props {
  connected: boolean;
  deviceInfo: DeviceInfo | null;
  onConnected: (info: DeviceInfo) => void;
  onDisconnected: () => void;
}

export default function ConnectPanel({
  connected,
  deviceInfo,
  onConnected,
  onDisconnected,
}: Props) {
  const [dllPath, setDllPath] = useState(
    "C:\\Program Files (x86)\\Drew Technologies, Inc\\J2534\\MongoosePro JLR\\monpj432.dll"
  );
  const [devices, setDevices] = useState<J2534DeviceEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [benchMode, setBenchMode] = useState(false);

  useEffect(() => {
    api.discoverDevices().then(setDevices).catch(() => {});
  }, []);

  const handleConnect = async () => {
    setLoading(true);
    setError(null);
    try {
      const info = await api.connect(dllPath);
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
      await api.toggleBenchMode(newValue);
      setBenchMode(newValue);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="space-y-4 max-w-xl">
      <h2 className="text-lg font-bold text-accent">J2534 Connection</h2>

      {/* DLL Path */}
      <div className="card space-y-3">
        <label className="block text-xs text-gray-400 uppercase tracking-wider">
          J2534 DLL Path
        </label>
        <input
          type="text"
          value={dllPath}
          onChange={(e) => setDllPath(e.target.value)}
          disabled={connected}
          className="w-full bg-bg-primary border border-gray-600 rounded px-3 py-2 text-sm font-mono
                     focus:border-accent focus:outline-none disabled:opacity-50"
          placeholder="Path to J2534 DLL..."
        />

        {devices.length > 0 && !connected && (
          <div>
            <label className="block text-xs text-gray-400 mb-1">
              Detected devices:
            </label>
            {devices.map((d) => (
              <button
                key={d.dll_path}
                onClick={() => setDllPath(d.dll_path)}
                className="block text-xs text-accent hover:underline"
              >
                {d.name}
              </button>
            ))}
          </div>
        )}

        <div className="flex gap-2">
          {!connected ? (
            <button
              onClick={handleConnect}
              disabled={loading || !dllPath}
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
        <div className="card">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-sm font-semibold text-gray-300">
                Bench Mode (BCM Emulation)
              </h3>
              <p className="text-xs text-gray-500 mt-0.5">
                Emulate BCM on CAN bus for bench testing without vehicle
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
                Not connected — plug in Mongoose Pro cable and click Connect
              </span>
            )}
          </span>
        </div>
      </div>
    </div>
  );
}
