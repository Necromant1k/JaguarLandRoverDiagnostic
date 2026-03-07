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

export interface CanSniffEntry {
  timestamp_ms: number;
  can_id: string;
  data_hex: string;
  data_len: number;
}

export interface CanSniffResult {
  routine_response: string | null;
  baseline_frames: CanSniffEntry[];
  after_frames: CanSniffEntry[];
  new_can_ids: string[];
  summary: string;
}

export interface RestoreCcfResult {
  success: boolean;
  steps: RestoreCcfStep[];
  pre_flight: PreFlightInfo | null;
  mid_flight: MidFlightInfo | null;
  post_flight: PostFlightInfo | null;
  sniff_frames: CanSniffEntry[];
}

export interface MidFlightInfo {
  imc_ccf_0e02_hex: string | null;
  imc_ccf_0e02_len: number;
  imc_ccf_0e01_hex: string | null;
  option_pairs: { option: number; value: number; value_hex: string }[];
  option_467_value: string | null;
}

export interface RestoreCcfStep {
  name: string;
  success: boolean;
  detail: string;
  duration_ms: number;
}

export interface PreFlightInfo {
  gwm_ccf_hex: string;
  option_467_raw: number | null;
  option_467_extracted: number | null;
  option_467_desc: string;
  warnings: string[];
}

export interface PostFlightInfo {
  dids_read: EcuInfoEntry[];
  imc_responsive: boolean;
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
