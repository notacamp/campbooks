// Pure helpers of the search screens: the query text and its filters, the
// device's recent searches, and what a hit says. No React, no fetching.
import { readPref, writePref } from "~/lib/storage/prefs";

import type { SearchFilter, SearchWarning } from "./api/queries";

/** `t` for keys only known at runtime (filter operators). */
export type Translate = (key: string, options?: Record<string, unknown>) => string;

/** The longest query the server accepts. */
export const MAX_QUERY = 512;

// ── the query text ───────────────────────────────────────────────────────

/** Splits on whitespace outside double quotes; keeps each token's own text. */
function tokenize(text: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let quoted = false;
  for (const char of text) {
    if (char === '"') {
      quoted = !quoted;
      current += char;
    } else if (/\s/u.test(char) && !quoted) {
      if (current !== "") {
        tokens.push(current);
      }
      current = "";
    } else {
      current += char;
    }
  }
  if (current !== "") {
    tokens.push(current);
  }
  return tokens;
}

const unquote = (value: string) => value.replaceAll('"', "");

/** A token as operator and value (`-From:"Ana L"` → from, ana l), or null for a plain word. */
function operatorToken(token: string): { operator: string; value: string } | null {
  const body = token.startsWith("-") ? token.slice(1) : token;
  const colon = body.indexOf(":");
  if (colon <= 0) {
    return null;
  }
  return {
    operator: body.slice(0, colon).toLowerCase(),
    value: unquote(body.slice(colon + 1)).toLowerCase(),
  };
}

/** Index of the token a parsed filter came from; -1 when none matches. */
function tokenOf(tokens: readonly string[], filter: SearchFilter): number {
  const operator = filter.operator.toLowerCase();
  const value = filter.value.toLowerCase();
  const parsed = tokens.map(operatorToken);
  const exact = parsed.findIndex((p) => p?.operator === operator && p.value === value);
  if (exact !== -1) {
    return exact;
  }
  // The server may normalise a value (a date, a case): fall back to the operator.
  return parsed.findIndex((p) => p?.operator === operator);
}

/** The query without the token a filter came from (the server stays the only parser). */
export function removeFilter(text: string, filter: SearchFilter): string {
  const tokens = tokenize(text);
  const index = tokenOf(tokens, filter);
  return index === -1 ? text : tokens.filter((_, other) => other !== index).join(" ");
}

/** The query with `token` appended, if it is not there already. */
export function addToken(text: string, token: string): string {
  const tokens = tokenize(text);
  return tokens.includes(token) ? text : [...tokens, token].join(" ");
}

/** Whether the parsed filters hold `operator:value` (a quick toggle's state). */
export function hasFilter(
  filters: readonly SearchFilter[],
  operator: string,
  value: string,
): boolean {
  return filters.some((filter) => filter.operator === operator && filter.value === value);
}

/** Toggles a quick filter: removes the filter the server parsed, or appends its token. */
export function toggleFilter(
  text: string,
  filters: readonly SearchFilter[],
  operator: string,
  value: string,
): string {
  const applied = filters.find((filter) => filter.operator === operator && filter.value === value);
  return applied === undefined
    ? addToken(text, `${operator}:${value}`)
    : removeFilter(text, applied);
}

/** The label of a filter chip, in the member's language. */
export function chipLabel(t: Translate, filter: SearchFilter): string {
  const { operator, value } = filter;
  if (operator === "has" && value === "attachment") {
    return t("chip.attachment");
  }
  if (operator === "is" && ["unread", "read", "starred"].includes(value)) {
    return t(`chip.is.${value}`);
  }
  if (["from", "to", "cc", "subject", "in", "label"].includes(operator)) {
    return t(`chip.${operator}`, { value });
  }
  if (["before", "after", "older_than", "newer_than"].includes(operator)) {
    return t(`chip.date.${operator}`, { value });
  }
  return t("chip.other", { operator, value });
}

/** Warnings worth a line: what was not understood. */
export function warningText(t: Translate, warning: SearchWarning): string {
  const known = ["unknown_operator", "empty_value", "invalid_value"].includes(warning.code);
  return t(`warning.${known ? warning.code : "other"}`, { operator: warning.operator });
}

// ── recent searches (on this device) ─────────────────────────────────────

const RECENT_MAX = 8;

/** The device's recent searches, newest first (empty when storage is unavailable). */
export function recentSearches(): string[] {
  try {
    const parsed: unknown = JSON.parse(readPref("searches") ?? "[]");
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string").slice(0, RECENT_MAX)
      : [];
  } catch {
    return [];
  }
}

/** Remembers a query that was run (typed text only, never a result). */
export function rememberSearch(text: string): void {
  const query = text.trim();
  if (query === "" || query.length > MAX_QUERY) {
    return;
  }
  const others = recentSearches().filter((other) => other.toLowerCase() !== query.toLowerCase());
  writePref("searches", JSON.stringify([query, ...others].slice(0, RECENT_MAX)));
}

export function forgetSearches(): void {
  writePref("searches", "[]");
}

// ── the list ─────────────────────────────────────────────────────────────

/** Keeps the keyboard cursor in view (jsdom has no scrolling). */
export function scrollIntoView(id: string): void {
  const element: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
    document.getElementById(id);
  element?.scrollIntoView?.({ block: "nearest" });
}
