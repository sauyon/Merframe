// Regenerates data/vendor_offers.json from warframe-public-export-plus.
// Run from the repository root: node crates/wf-data/scripts/vendor-offers.mjs
import { writeFileSync } from "node:fs";

const REPO = "calamity-inc/warframe-public-export-plus";
const OUT = new URL("../data/vendor_offers.json", import.meta.url);
const NAMED = [
  "ExportAnimals",
  "ExportArcanes",
  "ExportBoosterPacks",
  "ExportBoosters",
  "ExportBundles",
  "ExportCreditBundles",
  "ExportCustoms",
  "ExportDrones",
  "ExportEmailItems",
  "ExportFlavour",
  "ExportFusionBundles",
  "ExportGear",
  "ExportKeys",
  "ExportRailjackWeapons",
  "ExportRelics",
  "ExportResources",
  "ExportSentinels",
  "ExportTextIcons",
  "ExportUpgrades",
  "ExportVirtuals",
  "ExportWarframes",
  "ExportWeapons",
];

async function json(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`${url}: ${response.status}`);
  }
  return response.json();
}

const { sha } = await json(`https://api.github.com/repos/${REPO}/commits/HEAD`);
const file = (name) =>
  json(`https://raw.githubusercontent.com/${REPO}/${sha}/${name}.json`);

const [vendors, recipes, dict, ...named] = await Promise.all([
  file("ExportVendors"),
  file("ExportRecipes"),
  file("dict.en"),
  ...NAMED.map(file),
]);

const keys = new Map();
for (const table of named) {
  for (const [uniqueName, entry] of Object.entries(table)) {
    if (entry?.name && !keys.has(uniqueName)) {
      keys.set(uniqueName, entry.name);
    }
  }
}

function tidy(text) {
  return text.replace(/\s+/g, " ").trim();
}

function fallbackName(uniqueName) {
  const tail = uniqueName.split("/").at(-1);
  return tail.replace(/([a-z0-9])([A-Z])/g, "$1 $2");
}

function nameOf(uniqueName) {
  const key = keys.get(uniqueName);
  if (key) {
    return tidy(dict[key] ?? key);
  }
  const recipe = recipes[uniqueName];
  if (recipe) {
    return `${nameOf(recipe.resultType) ?? fallbackName(recipe.resultType)} Blueprint`;
  }
  return null;
}

const out = {};
for (const manifest of Object.keys(vendors).sort()) {
  const offers = [];
  const seen = new Set();
  for (const item of vendors[manifest].items) {
    if (!item.itemPrices?.length) {
      continue;
    }
    const uniqueName = item.storeItem.replace("/StoreItems/", "/");
    const offer = {
      item: uniqueName,
      name: nameOf(uniqueName) ?? fallbackName(uniqueName),
      quantity: item.quantity ?? 1,
      prices: item.itemPrices.map((price) => [price.ItemType, price.ItemCount]),
    };
    if (item.credits) {
      offer.credits = item.credits;
    }
    // Rotating bins repeat an offer; one copy is enough.
    const key = JSON.stringify(offer);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    offers.push(offer);
  }
  if (offers.length > 0) {
    out[manifest] = offers;
  }
}

// One offer per line keeps regenerated diffs readable.
const vendorLines = Object.entries(out).map(
  ([manifest, offers]) =>
    `  ${JSON.stringify(manifest)}: [\n${offers.map((offer) => `   ${JSON.stringify(offer)}`).join(",\n")}\n  ]`,
);
const source = `https://github.com/${REPO}/blob/${sha}/ExportVendors.json`;
writeFileSync(
  OUT,
  `{\n "source": ${JSON.stringify(source)},\n "vendors": {\n${vendorLines.join(",\n")}\n }\n}\n`,
);
console.log(`${Object.keys(out).length} vendors written to ${OUT.pathname}`);
