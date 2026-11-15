import { describe, expect, it } from "vitest";
import {
  ago,
  countdown,
  counted,
  dateTime,
  dayLabel,
  marketUrl,
  monthLabel,
  num,
  percent,
  plat,
  secondsUntil,
  spansYears,
} from "@/lib/format";

describe("num", () => {
  it("rounds and groups thousands", () => {
    expect(num(7192988.4)).toBe("7,192,988");
  });

  it("dash for missing", () => {
    expect(num(null)).toBe("-");
    expect(num(undefined)).toBe("-");
  });
});

describe("counted", () => {
  it("singular for one, plural otherwise", () => {
    expect(counted(1, "relic")).toBe("1 relic");
    expect(counted(0, "relic")).toBe("0 relics");
    expect(counted(1200, "relic")).toBe("1,200 relics");
  });

  it("irregular plural", () => {
    expect(counted(1, "hidden entry", "hidden entries")).toBe("1 hidden entry");
    expect(counted(2, "hidden entry", "hidden entries")).toBe(
      "2 hidden entries",
    );
    expect(counted(2, "item needs", "items need")).toBe("2 items need");
  });

  it("dash for missing", () => {
    expect(counted(null, "item")).toBe("- items");
    expect(counted(undefined, "item")).toBe("- items");
  });
});

describe("plat", () => {
  it("one decimal", () => {
    expect(plat(108.5)).toBe("108.5");
    expect(plat(86)).toBe("86.0");
  });

  it("dash for missing", () => {
    expect(plat(null)).toBe("-");
  });
});

describe("percent", () => {
  it("percent of fraction", () => {
    expect(percent(0.9891)).toBe("98.9%");
  });

  it("digits", () => {
    expect(percent(0.9413944666884638, 0)).toBe("94%");
  });

  it("never rounds up to the next step", () => {
    expect(percent(0.99999)).toBe("99.9%");
    expect(percent(604 / 607, 2)).toBe("99.5%");
    expect(percent(1)).toBe("100%");
  });

  it("dash for missing", () => {
    expect(percent(undefined)).toBe("-");
  });
});

describe("countdown", () => {
  it("ready at zero", () => {
    expect(countdown(0)).toBe("ready");
    expect(countdown(-5)).toBe("ready");
  });

  it("days and hours", () => {
    expect(countdown(90000)).toBe("1d 1h");
  });

  it("hours and padded minutes", () => {
    expect(countdown(3720)).toBe("1h 02m");
  });

  it("minutes and padded seconds", () => {
    expect(countdown(125)).toBe("2m 05s");
  });

  it("seconds only", () => {
    expect(countdown(42)).toBe("42s");
  });
});

describe("secondsUntil", () => {
  it("seconds between now and the instant", () => {
    const now = Date.parse("2026-09-15T11:59:00Z");
    expect(secondsUntil("2026-09-15T12:00:00Z", now)).toBe(60);
  });

  it("negative once passed", () => {
    const now = Date.parse("2026-09-15T12:00:30Z");
    expect(secondsUntil("2026-09-15T12:00:00Z", now)).toBe(-30);
  });
});

describe("ago", () => {
  it("never without a date", () => {
    expect(ago(null)).toBe("never");
    expect(ago("")).toBe("never");
  });

  it("minutes ago", () => {
    expect(ago(new Date(Date.now() - 120_000).toISOString())).toBe("2m ago");
  });

  it("hours ago", () => {
    expect(ago(new Date(Date.now() - 3 * 3_600_000).toISOString())).toBe(
      "3h ago",
    );
  });

  it("clamps future dates to zero", () => {
    expect(ago(new Date(Date.now() + 60_000).toISOString())).toBe("0s ago");
  });
});

describe("dateTime", () => {
  it("carries the year", () => {
    expect(dateTime("2026-03-04T10:30:00Z")).toContain("2026");
  });
});

describe("spansYears", () => {
  it("true across new year", () => {
    expect(spansYears(["2025-12-31", "2026-01-01"])).toBe(true);
  });

  it("false within one year", () => {
    expect(spansYears(["2026-01-01", "2026-06-30"])).toBe(false);
  });
});

describe("dayLabel", () => {
  it("day and month", () => {
    expect(dayLabel("2026-03-04", false)).toBe("04 Mar");
  });

  it("with year", () => {
    expect(dayLabel("2026-03-04", true)).toBe("04 Mar 2026");
  });
});

describe("monthLabel", () => {
  it("month only", () => {
    expect(monthLabel("2026-03-04", false)).toBe("Mar");
  });

  it("with year", () => {
    expect(monthLabel("2026-03-04", true)).toBe("Mar 2026");
  });
});

describe("marketUrl", () => {
  it("item page for a slug", () => {
    expect(marketUrl("axi_a20_relic")).toBe(
      "https://warframe.market/items/axi_a20_relic",
    );
  });
});
