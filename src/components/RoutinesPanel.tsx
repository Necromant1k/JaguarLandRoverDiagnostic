import { useState, useEffect, useMemo } from "react";
import * as api from "../lib/tauri";
import type { RoutineInfo, RoutineResponse } from "../types";

interface Props {
  connected: boolean;
}

const CATEGORY_ORDER = ["Diagnostics", "Configuration", "Recovery", "Advanced"];

export default function RoutinesPanel({ connected }: Props) {
  const [routines, setRoutines] = useState<RoutineInfo[]>([]);
  const [results, setResults] = useState<Record<number, RoutineResponse>>({});
  const [running, setRunning] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.listRoutines().then(setRoutines).catch(() => {});
  }, []);

  const grouped = useMemo(() => {
    const map = new Map<string, RoutineInfo[]>();
    for (const r of routines) {
      const cat = r.category || "Other";
      if (!map.has(cat)) map.set(cat, []);
      map.get(cat)!.push(r);
    }
    return CATEGORY_ORDER
      .filter((cat) => map.has(cat))
      .map((cat) => ({ category: cat, items: map.get(cat)! }));
  }, [routines]);

  const handleRun = async (routineId: number) => {
    setRunning(routineId);
    setError(null);
    try {
      const res = await api.runRoutine(routineId);
      setResults((prev) => ({ ...prev, [routineId]: res }));
    } catch (e) {
      setError(`Routine 0x${routineId.toString(16).toUpperCase()}: ${String(e)}`);
    } finally {
      setRunning(null);
    }
  };

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-semibold text-[#cccccc]">Routines</h3>

      {error && (
        <div className="bg-err/10 border border-err/30 rounded p-2 text-err text-xs">
          {error}
        </div>
      )}

      {grouped.map(({ category, items }) => (
        <div key={category} className="space-y-2">
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wider">
            {category}
          </h3>

          {items.map((r) => {
            const result = results[r.routine_id];
            const isRunning = running === r.routine_id;

            return (
              <div key={r.routine_id} className="card">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-accent font-mono text-xs">
                        0x{r.routine_id.toString(16).toUpperCase().padStart(4, "0")}
                      </span>
                      <span className="text-sm font-medium text-gray-200">
                        {r.name}
                      </span>
                      {r.needs_security && (
                        <span className="text-yellow-500 text-xs" title="Requires security access">
                          &#x1F512;
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-gray-500 mt-0.5">{r.description}</p>
                  </div>
                  <button
                    onClick={() => handleRun(r.routine_id)}
                    disabled={!connected || isRunning}
                    className="btn btn-primary text-xs"
                  >
                    {isRunning ? (
                      <span className="flex items-center gap-1">
                        <span className="status-led pending" />
                        Running...
                      </span>
                    ) : (
                      "Start"
                    )}
                  </button>
                </div>

                {result && (
                  <div
                    className={`mt-2 text-xs p-2 rounded ${
                      result.success
                        ? "bg-ok/10 text-ok"
                        : "bg-err/10 text-err"
                    }`}
                  >
                    {result.description}
                    {result.raw_data.length > 0 && (
                      <span className="block mt-1 font-mono text-gray-400">
                        Raw:{" "}
                        {result.raw_data
                          .map((b) => b.toString(16).toUpperCase().padStart(2, "0"))
                          .join(" ")}
                      </span>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      ))}

      {routines.length === 0 && (
        <div className="card text-center text-gray-500 text-sm py-8">
          Loading routines...
        </div>
      )}
    </div>
  );
}
