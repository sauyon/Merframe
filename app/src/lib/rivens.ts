import { percent } from "@/lib/format";
import type { AttributeGrade } from "@/types";

export function gradeVariant(grade: string) {
  if (grade === "S") {
    return "default";
  }
  if (grade === "A") {
    return "accent";
  }
  if (grade === "B") {
    return "secondary";
  }
  if (grade === "C") {
    return "muted";
  }
  return "warning";
}

export function gradeClasses(grade: string): { text: string; fill: string } {
  return grade === "S" || grade.startsWith("A")
    ? { text: "text-primary", fill: "bg-primary" }
    : { text: "text-muted-foreground", fill: "bg-muted-foreground" };
}

export function attributeAmount(unit: string | null, value: number): string {
  if (unit === "multiply") {
    return `x${value.toFixed(2)}`;
  }
  const suffix = unit === "percent" ? "%" : unit === "seconds" ? "s" : "";
  const shown = Number(value.toFixed(1));
  const sign = shown > 0 ? "+" : "";
  return `${sign}${shown.toFixed(1)}${suffix}`;
}

export function attributeValue(attribute: AttributeGrade): string | null {
  return attribute.display === null
    ? null
    : attributeAmount(attribute.unit, attribute.display);
}

export function attributeLabel(attribute: AttributeGrade): string {
  return attribute.name ?? attribute.tag;
}

export function attributeText(attribute: AttributeGrade): string {
  const name = attributeLabel(attribute);
  const value = attributeValue(attribute);
  return value === null
    ? `${name} ${percent(attribute.percentile, 0)}`
    : `${value} ${name}`;
}

export function rivenAuctionUrl(weaponSlug: string): string {
  const query = new URLSearchParams({
    type: "riven",
    weapon_url_name: weaponSlug,
    sort_by: "price_asc",
  });
  return `https://warframe.market/auctions/search?${query.toString()}`;
}
