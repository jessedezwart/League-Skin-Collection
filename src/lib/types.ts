export interface Skin {
  skinId: number;
  name: string;
  championId: number;
  championName: string;
  rp: number | null;
  valueSource: "store" | "estimate" | "none";
  rarity: string;
  isLegacy: boolean;
  availabilityDate: string | null;
}

export type SortKey = "value" | "name" | "champion" | "availability";

export const SORT_LABELS: Record<SortKey, string> = {
  value: "Value",
  name: "Name",
  champion: "Champion",
  availability: "Availability",
};

/** Skins missing an availability date sort last, whichever direction the rest go. */
function availabilityTime(skin: Skin): number {
  if (!skin.availabilityDate) return -Infinity;
  const t = Date.parse(skin.availabilityDate);
  return Number.isNaN(t) ? -Infinity : t;
}

export const SORTERS: Record<SortKey, (a: Skin, b: Skin) => number> = {
  value: (a, b) => (b.rp ?? -1) - (a.rp ?? -1),
  name: (a, b) => a.name.localeCompare(b.name),
  champion: (a, b) => a.championName.localeCompare(b.championName),
  availability: (a, b) => availabilityTime(b) - availabilityTime(a),
};

export function formatAvailabilityDate(iso: string | null): string | null {
  if (!iso) return null;
  const t = Date.parse(iso);
  return Number.isNaN(t)
    ? null
    : new Date(t).toLocaleDateString(undefined, {
        year: "numeric",
        month: "short",
        day: "numeric",
      });
}

export function availabilityYear(iso: string | null): string | null {
  if (!iso) return null;
  const t = Date.parse(iso);
  return Number.isNaN(t) ? null : String(new Date(t).getFullYear());
}

export interface Champion {
  championId: number;
  name: string;
}

export type ConnectionStatus =
  | { state: "notRunning" }
  | { state: "notLoggedIn" }
  | { state: "connected"; summonerName: string };

export const RARITY_LABELS: Record<string, string> = {
  kEpic: "Epic",
  kLegendary: "Legendary",
  kUltimate: "Ultimate",
  kMythic: "Mythic",
  kExalted: "Exalted",
  kTranscendent: "Transcendent",
  kRare: "Rare",
  kNoRarity: "",
};

export function rarityLabel(rarity: string): string {
  return RARITY_LABELS[rarity] ?? rarity.replace(/^k/, "");
}

export type Currency = "EUR" | "USD";

// Real-money rate from the smallest RP package (575 RP for 4.99), which has
// no bonus RP baked in. NA and EUW tiers use the same numbers, so the rate
// is identical for USD and EUR.
const COST_PER_RP = 4.99 / 575;

export const CURRENCY_SYMBOLS: Record<Currency, string> = {
  EUR: "€",
  USD: "$",
};

export function moneyValue(rp: number): number {
  return rp * COST_PER_RP;
}

export function formatMoney(rp: number, currency: Currency): string {
  return (
    CURRENCY_SYMBOLS[currency] +
    moneyValue(rp).toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    })
  );
}
