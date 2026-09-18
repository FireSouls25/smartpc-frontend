import { describe, expect, test } from "vitest";
import { contextPct, fmtK, timeOf } from "./format";

describe("timeOf", () => {
  test("today renders as HH:MM", () => {
    const noon = new Date();
    noon.setHours(12, 34, 0, 0);
    expect(timeOf(noon.toISOString())).toBe(
      noon.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
    );
  });

  test("older dates render as day + month", () => {
    const old = new Date("2020-01-15T12:00:00Z");
    expect(timeOf(old.toISOString())).toBe(
      old.toLocaleDateString([], { day: "2-digit", month: "short" }),
    );
  });

  test("garbage renders as empty", () => {
    expect(timeOf("not-a-date")).toBe("");
    expect(timeOf("")).toBe("");
  });
});

describe("fmtK", () => {
  test.each([
    [0, "0"],
    [999, "999"],
    [1000, "1.0K"],
    [8200, "8.2K"],
    [8192, "8.2K"],
  ])("%i → %s", (n, expected) => {
    expect(fmtK(n)).toBe(expected);
  });
});

describe("contextPct", () => {
  test("ratio, clamp and null-window", () => {
    expect(contextPct(4100, 8200)).toBe(50);
    expect(contextPct(0, 8200)).toBe(0);
    expect(contextPct(9000, 8000)).toBe(100);
    expect(contextPct(10, null)).toBe(0);
    expect(contextPct(10, 0)).toBe(0);
  });
});
