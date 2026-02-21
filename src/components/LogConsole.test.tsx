import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import LogConsole from "./LogConsole";
import type { LogEntry } from "../types";

describe("LogConsole", () => {
  const mockLogs: LogEntry[] = [
    {
      direction: "Tx",
      data_hex: "22 F1 90",
      timestamp: "12:00:00.000",
      description: "ReadDID VIN",
    },
    {
      direction: "Rx",
      data_hex: "62 F1 90 53 41 4A",
      timestamp: "12:00:00.100",
      description: "ReadDID VIN",
    },
    {
      direction: "Error",
      data_hex: "7F 22 31",
      timestamp: "12:00:01.000",
      description: "NRC: Request out of range",
    },
    {
      direction: "Pending",
      data_hex: "7F 31 78",
      timestamp: "12:00:02.000",
      description: "Response pending...",
    },
  ];

  it("renders log entries", () => {
    render(<LogConsole logs={mockLogs} onClear={vi.fn()} />);
    expect(screen.getByText("22 F1 90")).toBeInTheDocument();
    expect(screen.getByText("62 F1 90 53 41 4A")).toBeInTheDocument();
    expect(screen.getByText("7F 22 31")).toBeInTheDocument();
  });

  it("shows TX labels for transmit entries", () => {
    render(<LogConsole logs={mockLogs} onClear={vi.fn()} />);
    const txLabels = screen.getAllByText("TX");
    expect(txLabels.length).toBeGreaterThan(0);
  });

  it("shows RX labels for receive entries", () => {
    render(<LogConsole logs={mockLogs} onClear={vi.fn()} />);
    const rxLabels = screen.getAllByText("RX");
    expect(rxLabels.length).toBeGreaterThan(0);
  });

  it("shows ERR labels for error entries", () => {
    render(<LogConsole logs={mockLogs} onClear={vi.fn()} />);
    const errLabels = screen.getAllByText("ERR");
    expect(errLabels.length).toBeGreaterThan(0);
  });

  it("shows pending labels", () => {
    render(<LogConsole logs={mockLogs} onClear={vi.fn()} />);
    expect(screen.getByText("...")).toBeInTheDocument();
  });

  it("shows entry count", () => {
    render(<LogConsole logs={mockLogs} onClear={vi.fn()} />);
    expect(screen.getByText("4 entries")).toBeInTheDocument();
  });

  it("shows timestamps", () => {
    render(<LogConsole logs={mockLogs} onClear={vi.fn()} />);
    expect(screen.getByText("12:00:00.000")).toBeInTheDocument();
    expect(screen.getByText("12:00:00.100")).toBeInTheDocument();
  });

  it("renders clear button", () => {
    render(<LogConsole logs={mockLogs} onClear={vi.fn()} />);
    expect(screen.getByText("Clear")).toBeInTheDocument();
  });

  it("renders empty log", () => {
    render(<LogConsole logs={[]} onClear={vi.fn()} />);
    expect(screen.getByText("0 entries")).toBeInTheDocument();
  });

  it("applies correct color classes for TX entries", () => {
    render(<LogConsole logs={[mockLogs[0]]} onClear={vi.fn()} />);
    const txData = screen.getByText("22 F1 90");
    expect(txData.className).toContain("text-tx");
  });

  it("applies correct color classes for RX entries", () => {
    render(<LogConsole logs={[mockLogs[1]]} onClear={vi.fn()} />);
    const rxData = screen.getByText("62 F1 90 53 41 4A");
    expect(rxData.className).toContain("text-rx");
  });

  it("applies correct color classes for Error entries", () => {
    render(<LogConsole logs={[mockLogs[2]]} onClear={vi.fn()} />);
    const errData = screen.getByText("7F 22 31");
    expect(errData.className).toContain("text-err");
  });
});
