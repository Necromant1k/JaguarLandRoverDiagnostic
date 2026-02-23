export interface DeviceInfo {
  firmware_version: string;
  dll_version: string;
  api_version: string;
  dll_path: string;
}

export interface EcuInfoEntry {
  label: string;
  did_hex: string;
  value: string | null;
  error: string | null;
  category: string;
}

export interface RoutineInfo {
  routine_id: number;
  name: string;
  description: string;
  category: string;
  needs_security: boolean;
  needs_pending: boolean;
}

export interface RoutineResponse {
  success: boolean;
  description: string;
  raw_data: number[];
}

export interface J2534DeviceEntry {
  name: string;
  dll_path: string;
}

export interface LogEntry {
  direction: "Tx" | "Rx" | "Error" | "Pending";
  data_hex: string;
  timestamp: string;
  description: string;
}

export interface BenchModeStatus {
  enabled: boolean;
  emulated_ecus: string[];
}

export type Tab = "connect" | "imc" | "bcm" | "gwm" | "ipc";

export interface CcfCompareEntry {
  option_id: number;
  name: string;
  gwm: string | null;
  bcm: string | null;
  imc: string | null;
  mismatch: boolean;
}
