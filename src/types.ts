export interface DeviceInfo {
  firmware_version: string;
  dll_version: string;
  api_version: string;
  dll_path: string;
}

export interface VehicleInfo {
  vin: string | null;
  voltage: number | null;
  master_part: string | null;
  v850_part: string | null;
  tuner_part: string | null;
  serial: string | null;
  session: string | null;
}

export interface SshResult {
  success: boolean;
  ip_address: string;
  message: string;
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

export type Tab = "connect" | "vehicle" | "ssh" | "imc";
