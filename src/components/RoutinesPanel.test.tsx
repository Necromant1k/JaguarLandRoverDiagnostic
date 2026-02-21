import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import RoutinesPanel from "./RoutinesPanel";

const mockInvoke = vi.mocked(invoke);

const mockRoutines = [
  {
    routine_id: 0x0e00,
    name: "Retrieve CCF",
    description: "Retrieve Car Configuration File (0x0E00)",
    category: "Configuration",
    needs_security: false,
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
  {
    routine_id: 0x6038,
    name: "Configure Linux to Hardware",
    description: "Reconfigure IMC Linux environment (0x6038)",
    category: "Configuration",
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
      expect(screen.getByText("Retrieve CCF")).toBeInTheDocument();
      expect(screen.getByText("SSH Enable")).toBeInTheDocument();
      expect(screen.getByText("Configure Linux to Hardware")).toBeInTheDocument();
    });
  });

  it("shows routine IDs in hex", async () => {
    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("0x0E00")).toBeInTheDocument();
      expect(screen.getByText("0x603E")).toBeInTheDocument();
    });
  });

  it("shows lock icon for secured routines", async () => {
    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      // SSH Enable and Configure Linux need security, Retrieve CCF doesn't
      const locks = screen.getAllByTitle("Requires security access");
      expect(locks.length).toBe(2); // 603E and 6038
    });
  });

  it("shows start buttons when connected", async () => {
    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      const buttons = screen.getAllByText("Start");
      expect(buttons.length).toBe(3);
    });
  });

  it("disables start buttons when disconnected", async () => {
    render(<RoutinesPanel connected={false} />);

    await waitFor(() => {
      const buttons = screen.getAllByText("Start");
      buttons.forEach((btn) => {
        expect(btn).toBeDisabled();
      });
    });
  });

  it("shows success result after running routine", async () => {
    mockInvoke
      .mockResolvedValueOnce(mockRoutines) // list_routines
      .mockResolvedValueOnce({
        success: true,
        description: "Routine 0x0E00 OK: 10 01",
        raw_data: [0x10, 0x01],
      });

    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("Retrieve CCF")).toBeInTheDocument();
    });

    const buttons = screen.getAllByText("Start");
    // Click the first routine's Start button (Retrieve CCF is in Configuration category)
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      expect(screen.getByText(/Routine 0x0E00 OK/)).toBeInTheDocument();
    });
  });

  it("shows error when routine fails", async () => {
    mockInvoke
      .mockResolvedValueOnce(mockRoutines) // list_routines
      .mockRejectedValueOnce("TesterPresent failed: Timeout waiting for response");

    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("Retrieve CCF")).toBeInTheDocument();
    });

    const buttons = screen.getAllByText("Start");
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      expect(
        screen.getByText(/TesterPresent failed: Timeout waiting for response/)
      ).toBeInTheDocument();
    });
  });

  it("shows Running state while routine executes", async () => {
    // Never resolve the routine call — keep it pending
    let resolveRoutine: (v: unknown) => void;
    const routinePromise = new Promise((r) => {
      resolveRoutine = r;
    });

    mockInvoke
      .mockResolvedValueOnce(mockRoutines) // list_routines
      .mockReturnValueOnce(routinePromise as never); // run_routine — hangs

    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("Retrieve CCF")).toBeInTheDocument();
    });

    const buttons = screen.getAllByText("Start");
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      expect(screen.getByText("Running...")).toBeInTheDocument();
    });

    // Resolve to clean up
    resolveRoutine!({ success: true, description: "OK", raw_data: [] });
  });

  it("clears Running state after error", async () => {
    mockInvoke
      .mockResolvedValueOnce(mockRoutines)
      .mockRejectedValueOnce("Connection lost");

    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("Retrieve CCF")).toBeInTheDocument();
    });

    const buttons = screen.getAllByText("Start");
    fireEvent.click(buttons[0]);

    // After error, should show Start button again (not Running)
    await waitFor(() => {
      expect(screen.queryByText("Running...")).not.toBeInTheDocument();
      expect(screen.getByText(/Connection lost/)).toBeInTheDocument();
    });
  });

  it("shows failed result styling for unsuccessful routine", async () => {
    mockInvoke
      .mockResolvedValueOnce(mockRoutines)
      .mockResolvedValueOnce({
        success: false,
        description: "Routine 0x0E00 failed",
        raw_data: [],
      });

    render(<RoutinesPanel connected={true} />);

    await waitFor(() => {
      expect(screen.getByText("Retrieve CCF")).toBeInTheDocument();
    });

    const buttons = screen.getAllByText("Start");
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      const result = screen.getByText("Routine 0x0E00 failed");
      expect(result).toBeInTheDocument();
      // Should have error styling
      expect(result.closest("div")?.className).toContain("text-err");
    });
  });
});
