import { describe, expect, it } from "vitest";
import {
  attributeAmount,
  attributeLabel,
  attributeText,
  attributeValue,
  gradeClasses,
  gradeVariant,
  rivenAuctionUrl,
} from "@/lib/rivens";
import type { AttributeGrade } from "@/types";

function attribute(patch: Partial<AttributeGrade>): AttributeGrade {
  return {
    tag: "WeaponFireRateMod",
    name: "Fire Rate / Attack Speed",
    slug: null,
    unit: null,
    prefix: null,
    suffix: null,
    localization: null,
    value: 5,
    percentile: 0.5,
    grade: "B",
    multiplier: 1,
    rolled: 5,
    display: 5,
    min: 4.1,
    max: 5,
    curse: false,
    spliced: false,
    desired: false,
    ...patch,
  };
}

const initialCombo = attribute({
  tag: "WeaponMeleeComboInitialBonusMod",
  name: "Initial combo",
  value: 2.2,
  rolled: 2.2,
  display: 2.2,
  min: 1,
  max: 3,
});

const attackSpeed = attribute({
  unit: "percent",
  percentile: 0.91,
  grade: "A",
});

describe("gradeVariant", () => {
  it("variant per letter", () => {
    expect(gradeVariant("S")).toBe("default");
    expect(gradeVariant("A")).toBe("accent");
    expect(gradeVariant("B")).toBe("secondary");
    expect(gradeVariant("C")).toBe("muted");
    expect(gradeVariant("D")).toBe("warning");
  });
});

describe("gradeClasses", () => {
  it("primary for S and A", () => {
    expect(gradeClasses("S").text).toBe("text-primary");
    expect(gradeClasses("A-").fill).toBe("bg-primary");
  });

  it("muted otherwise", () => {
    expect(gradeClasses("B+").text).toBe("text-muted-foreground");
    expect(gradeClasses("F").fill).toBe("bg-muted-foreground");
  });
});

describe("attributeAmount", () => {
  it("percent with sign", () => {
    expect(attributeAmount("percent", 96.4)).toBe("+96.4%");
    expect(attributeAmount("percent", -42.3)).toBe("-42.3%");
  });

  it("seconds", () => {
    expect(attributeAmount("seconds", 1.2)).toBe("+1.2s");
  });

  it("multiplier", () => {
    expect(attributeAmount("multiply", 2.222)).toBe("x2.22");
  });

  it("plain number", () => {
    expect(attributeAmount(null, 2.2)).toBe("+2.2");
  });

  it("no sign on a rounded zero", () => {
    expect(attributeAmount("percent", 0.04)).toBe("0.0%");
    expect(attributeAmount("percent", -0.04)).toBe("0.0%");
  });
});

describe("attributeValue", () => {
  it("formats the display value", () => {
    expect(attributeValue(attackSpeed)).toBe("+5.0%");
  });

  it("null without a display value", () => {
    expect(attributeValue({ ...attackSpeed, display: null })).toBeNull();
  });
});

describe("attributeText", () => {
  it("value then name", () => {
    expect(attributeText(initialCombo)).toBe("+2.2 Initial combo");
  });

  it("name then percentile without a value", () => {
    expect(attributeText({ ...attackSpeed, display: null })).toBe(
      "Fire Rate / Attack Speed 91%",
    );
  });

  it("tag when the name is unknown", () => {
    expect(attributeText({ ...initialCombo, name: null })).toBe(
      "+2.2 WeaponMeleeComboInitialBonusMod",
    );
  });
});

describe("attributeLabel", () => {
  it("name, or the tag when the name is unknown", () => {
    expect(attributeLabel(initialCombo)).toBe("Initial combo");
    expect(attributeLabel(attribute({ name: null }))).toBe("WeaponFireRateMod");
  });
});

describe("rivenAuctionUrl", () => {
  it("cheapest auctions for the weapon", () => {
    expect(rivenAuctionUrl("kuva_shildeg")).toBe(
      "https://warframe.market/auctions/search?type=riven&weapon_url_name=kuva_shildeg&sort_by=price_asc",
    );
  });
});
