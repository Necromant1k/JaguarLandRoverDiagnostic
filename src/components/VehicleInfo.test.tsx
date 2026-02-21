import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import VehicleInfo from "./VehicleInfo";

const mockInvoke = vi.mocked(invoke);

describe("VehicleInfo", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders read button", () => {
    render(<VehicleInfo connected={true} />);
    expect(screen.getByText("Read Info")).toBeInTheDocument();
  });

  it("disables read button when disconnected", () => {
    render(<VehicleInfo connected={false} />);
    expect(screen.getByText("Read Info")).toBeDisabled();
  });

  it("displays vehicle info after read", async () => {
    mockInvoke.mockResolvedValue({
      vin: "SAJBA4BN0HA123456",
      voltage: 12.4,
      master_part: "GX63-14F012-AC",
      v850_part: "GX63-14F045-AB",
      tuner_part: "GX63-18K875-AA",
      serial: "123456789",
      session: "Default (0x01)",
    });

    render(<VehicleInfo connected={true} />);
    fireEvent.click(screen.getByText("Read Info"));

    await waitFor(() => {
      expect(screen.getByText("SAJBA4BN0HA123456")).toBeInTheDocument();
      expect(screen.getByText("12.4 V")).toBeInTheDocument();
      expect(screen.getByText("GX63-14F012-AC")).toBeInTheDocument();
      expect(screen.getByText("123456789")).toBeInTheDocument();
      expect(screen.getByText("Default (0x01)")).toBeInTheDocument();
    });
  });

  it("shows placeholder when no data loaded", () => {
    render(<VehicleInfo connected={true} />);
    expect(screen.getByText(/Click.*Read Info/)).toBeInTheDocument();
  });
});
