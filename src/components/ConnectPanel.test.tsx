import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import ConnectPanel from "./ConnectPanel";

const mockInvoke = vi.mocked(invoke);

describe("ConnectPanel", () => {
  const defaultProps = {
    connected: false,
    deviceInfo: null,
    onConnected: vi.fn(),
    onDisconnected: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue([]);
  });

  it("renders connect button when disconnected", () => {
    render(<ConnectPanel {...defaultProps} />);
    expect(screen.getByText("Connect")).toBeInTheDocument();
  });

  it("renders disconnect button when connected", () => {
    render(
      <ConnectPanel
        {...defaultProps}
        connected={true}
        deviceInfo={{
          firmware_version: "1.0",
          dll_version: "1.0",
          api_version: "04.04",
          dll_path: "test.dll",
        }}
      />
    );
    expect(screen.getByText("Disconnect")).toBeInTheDocument();
  });

  it("shows connected status LED when connected", () => {
    render(
      <ConnectPanel
        {...defaultProps}
        connected={true}
        deviceInfo={{
          firmware_version: "1.0",
          dll_version: "1.0",
          api_version: "04.04",
          dll_path: "test.dll",
        }}
      />
    );
    expect(screen.getByText("Connected to Mongoose Pro")).toBeInTheDocument();
  });

  it("shows disconnected status when not connected", () => {
    render(<ConnectPanel {...defaultProps} />);
    expect(
      screen.getByText(/Not connected/)
    ).toBeInTheDocument();
  });

  it("shows device info when connected", () => {
    render(
      <ConnectPanel
        {...defaultProps}
        connected={true}
        deviceInfo={{
          firmware_version: "2.3.4",
          dll_version: "5.6.7",
          api_version: "04.04",
          dll_path: "C:\\test\\test.dll",
        }}
      />
    );
    expect(screen.getByText("2.3.4")).toBeInTheDocument();
    expect(screen.getByText("5.6.7")).toBeInTheDocument();
    expect(screen.getByText("04.04")).toBeInTheDocument();
  });

  it("calls connect on button click", async () => {
    mockInvoke.mockResolvedValue({
      firmware_version: "1.0",
      dll_version: "1.0",
      api_version: "04.04",
      dll_path: "test.dll",
    });

    render(<ConnectPanel {...defaultProps} />);
    fireEvent.click(screen.getByText("Connect"));

    await waitFor(() => {
      expect(defaultProps.onConnected).toHaveBeenCalled();
    });
  });

  it("shows error on failed connection", async () => {
    mockInvoke.mockRejectedValueOnce([]).mockRejectedValue("Connection failed");

    render(<ConnectPanel {...defaultProps} />);
    fireEvent.click(screen.getByText("Connect"));

    await waitFor(() => {
      expect(screen.getByText("Connection failed")).toBeInTheDocument();
    });
  });
});
