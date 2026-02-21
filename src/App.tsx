import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import Sidebar from "./components/Sidebar";
import ConnectPanel from "./components/ConnectPanel";
import VehicleInfo from "./components/VehicleInfo";
import SshPanel from "./components/SshPanel";
import RoutinesPanel from "./components/RoutinesPanel";
import LogConsole from "./components/LogConsole";
import type { Tab, LogEntry, DeviceInfo } from "./types";

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("connect");
  const [connected, setConnected] = useState(false);
  const [deviceInfo, setDeviceInfo] = useState<DeviceInfo | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);

  const addLog = useCallback((entry: LogEntry) => {
    setLogs((prev) => [...prev.slice(-500), entry]);
  }, []);

  useEffect(() => {
    const unlisten = listen<LogEntry>("uds-log", (event) => {
      addLog(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [addLog]);

  const handleConnected = (info: DeviceInfo) => {
    setConnected(true);
    setDeviceInfo(info);
  };

  const handleDisconnected = () => {
    setConnected(false);
    setDeviceInfo(null);
  };

  const renderPanel = () => {
    switch (activeTab) {
      case "connect":
        return (
          <ConnectPanel
            connected={connected}
            deviceInfo={deviceInfo}
            onConnected={handleConnected}
            onDisconnected={handleDisconnected}
          />
        );
      case "vehicle":
        return <VehicleInfo connected={connected} />;
      case "ssh":
        return <SshPanel connected={connected} />;
      case "imc":
        return <RoutinesPanel connected={connected} />;
    }
  };

  return (
    <div className="h-screen flex flex-col bg-bg-primary">
      {/* Header */}
      <header className="h-10 flex items-center px-4 bg-bg-secondary border-b border-gray-700/50 shrink-0">
        <span className="text-accent font-bold text-sm tracking-wider">
          JLR UDS DIAGNOSTIC TOOL
        </span>
        <span className="ml-3 text-gray-500 text-xs">X260 / Jaguar XF/XE</span>
        <div className="ml-auto flex items-center gap-2">
          <span
            className={`status-led ${connected ? "connected" : "disconnected"}`}
          />
          <span className="text-xs text-gray-400">
            {connected ? "Connected" : "Disconnected"}
          </span>
        </div>
      </header>

      {/* Main area */}
      <div className="flex flex-1 min-h-0">
        <Sidebar activeTab={activeTab} onTabChange={setActiveTab} connected={connected} />
        <main className="flex-1 overflow-auto p-4">{renderPanel()}</main>
      </div>

      {/* Log console */}
      <LogConsole logs={logs} onClear={() => setLogs([])} />
    </div>
  );
}

export default App;
