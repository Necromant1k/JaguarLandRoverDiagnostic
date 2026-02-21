import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

const mockListen = vi.mocked(listen);
const mockInvoke = vi.mocked(invoke);

describe("App", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue([]);
    // Default: listen captures the handler but doesn't fire events
    mockListen.mockImplementation(() => Promise.resolve(() => {}));
  });

  it("renders header and log console", () => {
    render(<App />);
    expect(screen.getByText("JLR UDS DIAGNOSTIC TOOL")).toBeInTheDocument();
    expect(screen.getByText("UDS Log")).toBeInTheDocument();
    expect(screen.getByText("0 entries")).toBeInTheDocument();
  });

  it("subscribes to uds-log events on mount", () => {
    render(<App />);
    expect(mockListen).toHaveBeenCalledWith("uds-log", expect.any(Function));
  });

  it("displays log entries received from backend events", async () => {
    // Capture the event handler when listen is called
    let eventHandler: ((event: { payload: unknown }) => void) | null = null;
    mockListen.mockImplementation((_event, handler) => {
      eventHandler = handler as (event: { payload: unknown }) => void;
      return Promise.resolve(() => {});
    });

    render(<App />);

    // Verify listener was registered
    expect(eventHandler).not.toBeNull();

    // Simulate backend emitting a log event
    eventHandler!({
      payload: {
        direction: "Tx",
        data_hex: "3E 00",
        timestamp: "12:00:00.000",
        description: "TesterPresent",
      },
    });

    await waitFor(() => {
      expect(screen.getByText("3E 00")).toBeInTheDocument();
      expect(screen.getByText("TesterPresent")).toBeInTheDocument();
      expect(screen.getByText("1 entries")).toBeInTheDocument();
    });
  });

  it("displays multiple log entries in order", async () => {
    let eventHandler: ((event: { payload: unknown }) => void) | null = null;
    mockListen.mockImplementation((_event, handler) => {
      eventHandler = handler as (event: { payload: unknown }) => void;
      return Promise.resolve(() => {});
    });

    render(<App />);

    // Simulate TX + RX sequence
    eventHandler!({
      payload: {
        direction: "Tx",
        data_hex: "22 F1 90",
        timestamp: "12:00:00.000",
        description: "ReadDID VIN",
      },
    });
    eventHandler!({
      payload: {
        direction: "Rx",
        data_hex: "62 F1 90 53 41 4A",
        timestamp: "12:00:00.100",
        description: "",
      },
    });

    await waitFor(() => {
      expect(screen.getByText("22 F1 90")).toBeInTheDocument();
      expect(screen.getByText("62 F1 90 53 41 4A")).toBeInTheDocument();
      expect(screen.getByText("2 entries")).toBeInTheDocument();
    });
  });

  it("displays error log entries from backend", async () => {
    let eventHandler: ((event: { payload: unknown }) => void) | null = null;
    mockListen.mockImplementation((_event, handler) => {
      eventHandler = handler as (event: { payload: unknown }) => void;
      return Promise.resolve(() => {});
    });

    render(<App />);

    eventHandler!({
      payload: {
        direction: "Error",
        data_hex: "7F 31 22",
        timestamp: "12:00:01.000",
        description: "NRC: conditionsNotCorrect",
      },
    });

    await waitFor(() => {
      expect(screen.getByText("7F 31 22")).toBeInTheDocument();
      expect(screen.getByText("NRC: conditionsNotCorrect")).toBeInTheDocument();
    });
  });

  it("shows Disconnected status by default", () => {
    render(<App />);
    expect(screen.getByText("Disconnected")).toBeInTheDocument();
  });
});
