import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import RoutinesPanel from "./RoutinesPanel";

const mockInvoke = vi.mocked(invoke);

const mockRoutines = [
  {
    routine_id: 0x6038,
    name: "Configure Linux to Hardware",
    description: "Reconfigure IMC Linux environment (0x6038)",
    category: "Configuration",
    needs_security: true,
    needs_pending: true,
  },
  {
    routine_id: 0x603e,
    name: "SSH Enable",
    description: "Enable SSH access on IMC (0x603E)",
    category: "Diagnostics",
    needs_security: true,
    needs_pending: true,
  },
];

describe("RoutinesPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(mockRoutines);
  });

  it("renders routine list", async () => {
    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("Configure Linux to Hardware")).toBeInTheDocument();
      expect(screen.getByText("SSH Enable")).toBeInTheDocument();
    });
  });

  it("shows routine IDs in hex", async () => {
    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("0x6038")).toBeInTheDocument();
      expect(screen.getByText("0x603E")).toBeInTheDocument();
    });
  });

  it("shows start buttons", async () => {
    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      const buttons = screen.getAllByText("Start");
      expect(buttons.length).toBeGreaterThan(0);
    });
  });

  it("shows result after running routine", async () => {
    // First call returns routines, second returns routine result
    mockInvoke
      .mockResolvedValueOnce(mockRoutines)
      .mockResolvedValue({
        success: true,
        description: "Routine 0x6038 OK",
        raw_data: [0x10, 0x01, 0x00],
      });

    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("Configure Linux to Hardware")).toBeInTheDocument();
    });

    const buttons = screen.getAllByText("Start");
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      expect(screen.getByText(/Routine 0x6038 OK/)).toBeInTheDocument();
    });
  });
});
