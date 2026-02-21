import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { resolve } from "path";

describe("Tauri capabilities", () => {
  const capPath = resolve(__dirname, "../src-tauri/capabilities/default.json");

  it("capabilities file exists", () => {
    const content = readFileSync(capPath, "utf-8");
    expect(content.length).toBeGreaterThan(0);
  });

  it("has core:event:default permission (required for log events)", () => {
    const cap = JSON.parse(readFileSync(capPath, "utf-8"));
    expect(cap.permissions).toContain("core:event:default");
  });

  it("has core:default permission", () => {
    const cap = JSON.parse(readFileSync(capPath, "utf-8"));
    expect(cap.permissions).toContain("core:default");
  });

  it("targets main window", () => {
    const cap = JSON.parse(readFileSync(capPath, "utf-8"));
    expect(cap.windows).toContain("main");
  });
});
