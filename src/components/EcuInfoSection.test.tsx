import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import EcuInfoSection from "./EcuInfoSection";

const mockInvoke = vi.mocked(invoke);

describe("EcuInfoSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("auto-reads ECU info when connected", async () => {
    mockInvoke.mockResolvedValue([
      { label: "VIN", did_hex: "F190", value: "SAJBA4BN0HA123456", error: null },
      { label: "Battery Voltage", did_hex: "402A", value: "12.4 V", error: null },
    ]);

    render(<EcuInfoSection ecuId="bcm" connected={true} />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("read_ecu_info", { ecu: "bcm" });
      expect(screen.getByText("SAJBA4BN0HA123456")).toBeInTheDocument();
      expect(screen.getByText("12.4 V")).toBeInTheDocument();
    });
  });

  it("shows DID error inline", async () => {
    mockInvoke.mockResolvedValue([
      { label: "VIN", did_hex: "F190", value: null, error: "NRC: serviceNotSupported" },
    ]);

    render(<EcuInfoSection ecuId="imc" connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("NRC: serviceNotSupported")).toBeInTheDocument();
    });
  });

  it("does not read when disconnected", () => {
    render(<EcuInfoSection ecuId="imc" connected={false} />);
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(screen.getByText("Connect to read ECU info")).toBeInTheDocument();
  });

  it("has a refresh button", async () => {
    mockInvoke.mockResolvedValue([
      { label: "VIN", did_hex: "F190", value: "VIN1", error: null },
    ]);

    render(<EcuInfoSection ecuId="imc" connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("VIN1")).toBeInTheDocument();
    });

    // Update mock for refresh
    mockInvoke.mockResolvedValue([
      { label: "VIN", did_hex: "F190", value: "VIN2", error: null },
    ]);

    fireEvent.click(screen.getByText("Refresh"));

    await waitFor(() => {
      expect(screen.getByText("VIN2")).toBeInTheDocument();
    });
  });

  it("shows error when read fails entirely", async () => {
    mockInvoke.mockRejectedValue("Not connected");

    render(<EcuInfoSection ecuId="bcm" connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("Not connected")).toBeInTheDocument();
    });
  });
});
