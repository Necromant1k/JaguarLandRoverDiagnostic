import { invoke } from "@tauri-apps/api/core";
import type {
  DeviceInfo,
  EcuInfoEntry,
  RoutineInfo,
  RoutineResponse,
  J2534DeviceEntry,
  BenchModeStatus,
} from "../types";

export async function discoverDevices(): Promise<J2534DeviceEntry[]> {
  return invoke<J2534DeviceEntry[]>("discover_devices");
}

export async function connect(dllPath?: string): Promise<DeviceInfo> {
  return invoke<DeviceInfo>("connect", { dllPath });
}

export async function disconnect(): Promise<void> {
  return invoke<void>("disconnect");
}

export async function toggleBenchMode(
  enabled: boolean,
  ecus?: string[]
): Promise<void> {
  return invoke<void>("toggle_bench_mode", { enabled, ecus });
}

export async function getBenchModeStatus(): Promise<BenchModeStatus> {
  return invoke<BenchModeStatus>("get_bench_mode_status");
}

export async function readEcuInfo(ecu: string): Promise<EcuInfoEntry[]> {
  return invoke<EcuInfoEntry[]>("read_ecu_info", { ecu });
}

export async function runRoutine(
  routineId: number,
  data: number[] = []
): Promise<RoutineResponse> {
  return invoke<RoutineResponse>("run_routine", { routineId, data });
}

export async function readCcf(): Promise<EcuInfoEntry[]> {
  return invoke<EcuInfoEntry[]>("read_ccf");
}

export async function readDid(
  ecuTx: number,
  didId: number
): Promise<number[]> {
  return invoke<number[]>("read_did", { ecuTx, didId });
}

export async function listRoutines(): Promise<RoutineInfo[]> {
  return invoke<RoutineInfo[]>("list_routines");
}

export async function exportLogs(): Promise<string> {
  return invoke<string>("export_logs");
}
