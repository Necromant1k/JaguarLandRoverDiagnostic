import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import SshPanel from "./SshPanel";

const mockInvoke = vi.mocked(invoke);

describe("SshPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders enable SSH button", () => {
    render(<SshPanel connected={true} />);
    expect(screen.getByText("Enable SSH")).toBeInTheDocument();
  });

  it("disables button when disconnected", () => {
    render(<SshPanel connected={false} />);
    expect(screen.getByText("Enable SSH")).toBeDisabled();
  });

  it("shows success result with IP address", async () => {
    mockInvoke.mockResolvedValue({
      success: true,
      ip_address: "192.168.103.11",
      message: "SSH ENABLED — Connect: root@192.168.103.11",
    });

    render(<SshPanel connected={true} />);
    fireEvent.click(screen.getByText("Enable SSH"));

    await waitFor(() => {
      expect(screen.getByText("SSH ENABLED")).toBeInTheDocument();
      expect(screen.getByText(/root@192\.168\.103\.11/)).toBeInTheDocument();
    });
  });

  it("shows error on failure", async () => {
    mockInvoke.mockRejectedValue("Security access denied");

    render(<SshPanel connected={true} />);
    fireEvent.click(screen.getByText("Enable SSH"));

    await waitFor(() => {
      expect(screen.getByText("Security access denied")).toBeInTheDocument();
    });
  });

  it("shows protocol info", () => {
    render(<SshPanel connected={true} />);
    expect(screen.getByText(/TX: 0x7B3, RX: 0x7BB/)).toBeInTheDocument();
    expect(screen.getByText(/KeyGenMkI/)).toBeInTheDocument();
  });
});
