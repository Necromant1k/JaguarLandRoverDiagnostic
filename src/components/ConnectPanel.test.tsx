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

  const connectedDeviceInfo = {
    firmware_version: "1.0",
    dll_version: "1.0",
    api_version: "04.04",
    dll_path: "C:\\test\\test.dll",
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue([]);
  });

  it("renders connect button when disconnected", () => {
    render(<ConnectPanel {...defaultProps} />);
    expect(screen.getByText("Connect")).toBeInTheDocument();
  });

  it("renders device dropdown with auto-detect option", () => {
    render(<ConnectPanel {...defaultProps} />);
    const select = screen.getByRole("combobox");
    expect(select).toBeInTheDocument();
    expect(screen.getByText("Auto-detect (first available)")).toBeInTheDocument();
    expect(screen.getByText("Custom DLL path...")).toBeInTheDocument();
  });

  it("shows discovered devices in dropdown", async () => {
    mockInvoke.mockResolvedValueOnce([
      { name: "MongoosePro JLR", dll_path: "C:\\test\\mongo.dll" },
      { name: "Bosch VCI", dll_path: "C:\\test\\bosch.dll" },
    ]);

    render(<ConnectPanel {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText("MongoosePro JLR")).toBeInTheDocument();
      expect(screen.getByText("Bosch VCI")).toBeInTheDocument();
    });
  });

  it("shows manual path input when Custom selected", async () => {
    render(<ConnectPanel {...defaultProps} />);
    const select = screen.getByRole("combobox");
    fireEvent.change(select, { target: { value: "__manual__" } });

    expect(screen.getByPlaceholderText(/Program Files/)).toBeInTheDocument();
  });

  it("renders disconnect button when connected", () => {
    render(
      <ConnectPanel {...defaultProps} connected={true} deviceInfo={connectedDeviceInfo} />
    );
    expect(screen.getByText("Disconnect")).toBeInTheDocument();
  });

  it("shows connected status LED when connected", () => {
    render(
      <ConnectPanel {...defaultProps} connected={true} deviceInfo={connectedDeviceInfo} />
    );
    expect(screen.getByText("Connected to Mongoose Pro")).toBeInTheDocument();
  });

  it("shows disconnected status when not connected", () => {
    render(<ConnectPanel {...defaultProps} />);
    expect(screen.getByText(/Not connected/)).toBeInTheDocument();
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
    expect(screen.getByText("test.dll")).toBeInTheDocument();
  });

  it("calls connect on button click", async () => {
    const deviceInfo = {
      firmware_version: "1.0",
      dll_version: "1.0",
      api_version: "04.04",
      dll_path: "test.dll",
    };
    mockInvoke
      .mockResolvedValueOnce([]) // discover_devices
      .mockResolvedValueOnce(deviceInfo); // connect

    render(<ConnectPanel {...defaultProps} />);
    fireEvent.click(screen.getByText("Connect"));

    await waitFor(() => {
      expect(defaultProps.onConnected).toHaveBeenCalledWith(deviceInfo);
    });
  });

  it("shows error on failed connection", async () => {
    mockInvoke
      .mockResolvedValueOnce([]) // discover_devices
      .mockRejectedValueOnce("Connection failed"); // connect

    render(<ConnectPanel {...defaultProps} />);
    fireEvent.click(screen.getByText("Connect"));

    await waitFor(() => {
      expect(screen.getByText("Connection failed")).toBeInTheDocument();
    });
  });

  it("shows per-ECU checkboxes when connected", () => {
    render(
      <ConnectPanel {...defaultProps} connected={true} deviceInfo={connectedDeviceInfo} />
    );
    expect(screen.getByText("BCM")).toBeInTheDocument();
    expect(screen.getByText("GWM")).toBeInTheDocument();
    expect(screen.getByText("IPC")).toBeInTheDocument();
    expect(screen.getByText("Bench Mode (ECU Emulation)")).toBeInTheDocument();
  });

  // ---- BENCH MODE PERSISTENCE TESTS ----
  // These test the bug where bench mode resets when switching tabs

  it("loads bench mode status from backend on mount when connected", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "discover_devices") return [];
      if (cmd === "get_bench_mode_status")
        return { enabled: true, emulated_ecus: ["bcm", "gwm"] };
      return undefined;
    });

    render(
      <ConnectPanel {...defaultProps} connected={true} deviceInfo={connectedDeviceInfo} />
    );

    await waitFor(() => {
      // Verify get_bench_mode_status was called
      expect(mockInvoke).toHaveBeenCalledWith("get_bench_mode_status");
    });
  });

  it("restores bench mode ON state after remount (tab switch)", async () => {
    // Simulate: backend says bench mode is enabled with bcm+gwm
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "discover_devices") return [];
      if (cmd === "get_bench_mode_status")
        return { enabled: true, emulated_ecus: ["bcm", "gwm"] };
      return undefined;
    });

    const { unmount } = render(
      <ConnectPanel {...defaultProps} connected={true} deviceInfo={connectedDeviceInfo} />
    );

    // Wait for status to load
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_bench_mode_status");
    });

    // Unmount (user switches tab)
    unmount();

    // Remount (user comes back to connect tab)
    render(
      <ConnectPanel {...defaultProps} connected={true} deviceInfo={connectedDeviceInfo} />
    );

    // Should call get_bench_mode_status again on remount
    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(
        (c) => c[0] === "get_bench_mode_status"
      );
      expect(calls.length).toBeGreaterThanOrEqual(2);
    });
  });

  it("does not load bench mode status when disconnected", () => {
    render(<ConnectPanel {...defaultProps} connected={false} />);

    const calls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "get_bench_mode_status"
    );
    expect(calls.length).toBe(0);
  });
});
