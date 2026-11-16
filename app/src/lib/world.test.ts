import { describe, expect, it } from "vitest";
import {
  capitalize,
  elapsedShare,
  nodeLabel,
  splitNode,
  timeLeft,
} from "@/lib/world";

describe("capitalize", () => {
  it("upper cases the first letter", () => {
    expect(capitalize("fass")).toBe("Fass");
  });
});

describe("timeLeft", () => {
  it("0s when expired", () => {
    expect(timeLeft(0)).toBe("0s");
    expect(timeLeft(-10)).toBe("0s");
  });

  it("countdown otherwise", () => {
    expect(timeLeft(3720)).toBe("1h 02m");
  });
});

describe("splitNode", () => {
  it("splits planet and node", () => {
    expect(splitNode("Apollodorus (Mercury)", "SolNode23")).toEqual({
      node: "Apollodorus",
      planet: "Mercury",
    });
  });

  it("node id when the name is unknown", () => {
    expect(splitNode(null, "SolNode23")).toEqual({
      node: "SolNode23",
      planet: "",
    });
  });

  it("name without planet", () => {
    expect(splitNode("Zariman", "SolNode230")).toEqual({
      node: "Zariman",
      planet: "",
    });
  });
});

describe("nodeLabel", () => {
  it("node, planet", () => {
    expect(nodeLabel("Apollodorus (Mercury)", "SolNode23")).toBe(
      "Apollodorus, Mercury",
    );
  });

  it("falls back to the node id", () => {
    expect(nodeLabel(null, "SolNode23")).toBe("SolNode23");
  });
});

describe("elapsedShare", () => {
  it("halfway through", () => {
    expect(
      elapsedShare(
        "2026-09-15T00:00:00Z",
        "2026-09-16T00:00:00Z",
        Date.parse("2026-09-15T12:00:00Z"),
      ),
    ).toBe(50);
  });

  it("clamped to the range", () => {
    const starts = "2026-09-15T00:00:00Z";
    const ends = "2026-09-16T00:00:00Z";
    expect(elapsedShare(starts, ends, Date.parse("2026-09-14T00:00:00Z"))).toBe(
      0,
    );
    expect(elapsedShare(starts, ends, Date.parse("2026-09-17T00:00:00Z"))).toBe(
      100,
    );
  });

  it("complete when the window is empty", () => {
    expect(
      elapsedShare("2026-09-15T00:00:00Z", "2026-09-15T00:00:00Z", 0),
    ).toBe(100);
  });
});
