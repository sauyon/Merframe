import { describe, expect, it } from "vitest";
import { flipped, isShown } from "@/components/buff-filters";
import type { SeenBuff } from "@/types";

const filters = {
  buffs_squad: false,
  buffs_squad_shown: ["WispFrameReservoir"],
  buffs_hidden: [],
};
const mote: SeenBuff = {
  name: "WispFrameReservoirAbilitySpeedMote",
  kind: "squad",
  icon: null,
};
const roar: SeenBuff = {
  name: "RhinoRoarAbilityName",
  kind: "squad",
  icon: null,
};
const grace: SeenBuff = { name: "ArcaneGraceBuff", kind: "own", icon: null };

describe("buff filters", () => {
  it("shows only listed squad buffs while squad buffs are off", () => {
    expect(isShown(mote, filters)).toBe(true);
    expect(isShown(roar, filters)).toBe(false);
    expect(isShown(grace, filters)).toBe(true);
    expect(isShown(roar, { ...filters, buffs_squad: true })).toBe(true);
  });

  it("lists an off squad buff when clicked, and hides it on the next click", () => {
    const shown = { ...filters, ...flipped(roar, filters) };
    expect(shown.buffs_squad_shown).toContain(roar.name);
    expect(isShown(roar, shown)).toBe(true);
    const hidden = { ...shown, ...flipped(roar, shown) };
    expect(isShown(roar, hidden)).toBe(false);
    const again = { ...hidden, ...flipped(roar, hidden) };
    expect(isShown(roar, again)).toBe(true);
  });

  it("hides and unhides other buffs by name", () => {
    const hidden = { ...filters, ...flipped(grace, filters) };
    expect(hidden.buffs_hidden).toEqual([grace.name]);
    expect(flipped(grace, hidden)).toEqual({ buffs_hidden: [] });
  });
});
