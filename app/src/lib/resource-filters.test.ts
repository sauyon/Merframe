import { describe, expect, it } from "vitest";
import { visibleHoldings, visibleResources } from "@/lib/resource-filters";
import type { HeldResource, ResourceRow } from "@/types";

function resource(name: string, owned: number, required: number): ResourceRow {
  return {
    unique_name: `/Lotus/Types/Items/MiscItems/${name.replaceAll(" ", "")}`,
    name,
    image_name: null,
    owned,
    required,
    deficit: Math.max(required - owned, 0),
    used_by: [],
  };
}

const resources = [
  resource("Orokin Cell", 40, 30),
  resource("Hexenon", 5, 25),
  resource("Alloy Plate", 900, 600),
  resource("Neurodes", 1, 12),
];

function names(rows: { name: string }[]): string[] {
  return rows.map((row) => row.name);
}

describe("visibleResources", () => {
  it("puts the largest shortfall first, then sorts by name", () => {
    expect(names(visibleResources(resources, "all", ""))).toEqual([
      "Hexenon",
      "Neurodes",
      "Alloy Plate",
      "Orokin Cell",
    ]);
  });

  it("keeps only resources that are short", () => {
    expect(names(visibleResources(resources, "short", ""))).toEqual([
      "Hexenon",
      "Neurodes",
    ]);
  });

  it("matches the search anywhere in the name, ignoring case and padding", () => {
    expect(names(visibleResources(resources, "all", "  NEUR "))).toEqual([
      "Neurodes",
    ]);
    expect(visibleResources(resources, "short", "orokin")).toEqual([]);
  });

  it("leaves the payload order untouched", () => {
    visibleResources(resources, "all", "");
    expect(names(resources)[0]).toBe("Orokin Cell");
  });
});

function holding(name: string, owned: number, currency: boolean): HeldResource {
  return {
    unique_name: `/Lotus/Types/Items/MiscItems/${name.replaceAll(" ", "")}`,
    name,
    image_name: null,
    owned,
    vendors: currency
      ? [
          {
            id: "Vendor",
            vendor: "Vendor",
            location: "Relay",
            wiki_url: null,
            offers: [],
          },
        ]
      : [],
  };
}

const holdings = [
  holding("Vestigial Motes", 388, true),
  holding("Ferrite", 900, false),
  holding("Steel Essence", 0, true),
  holding("Entrati Obols", 12, true),
];

describe("visibleHoldings", () => {
  it("lists every held resource by name", () => {
    expect(names(visibleHoldings(holdings, "held", ""))).toEqual([
      "Entrati Obols",
      "Ferrite",
      "Vestigial Motes",
    ]);
  });

  it("lists only the vendor currencies that are held", () => {
    expect(names(visibleHoldings(holdings, "currencies", ""))).toEqual([
      "Entrati Obols",
      "Vestigial Motes",
    ]);
  });

  it("matches the search", () => {
    expect(names(visibleHoldings(holdings, "held", " MOTES "))).toEqual([
      "Vestigial Motes",
    ]);
  });
});
