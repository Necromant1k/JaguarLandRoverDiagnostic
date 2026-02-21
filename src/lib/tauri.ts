import { invoke } from "@tauri-apps/api/core";
import type {
  DeviceInfo,
  VehicleInfo,
  SshResult,
  RoutineInfo,
  RoutineResponse,
  J2534DeviceEntry,
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

export async function toggleBenchMode(enabled: boolean): Promise<void> {
  return invoke<void>("toggle_bench_mode", { enabled });
}

export async function readVehicleInfo(): Promise<VehicleInfo> {
  return invoke<VehicleInfo>("read_vehicle_info");
}

export async function enableSsh(): Promise<SshResult> {
  return invoke<SshResult>("enable_ssh");
}

export async function runRoutine(
  routineId: number,
  data: number[] = []
): Promise<RoutineResponse> {
  return invoke<RoutineResponse>("run_routine", { routineId, data });
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
