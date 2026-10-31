import { joinNames, type Readers } from "./readers";
import type { Refusal } from "./refusals";

/** `t` for keys only known at runtime. */
export type Translate = (key: string, options?: Record<string, unknown>) => string;

/** A name to search by: no accents, no capitals. */
export const fold = (text: string) =>
  text.normalize("NFD").replaceAll(/\p{M}/gu, "").toLocaleLowerCase();

/** "Rui and 2 others will read it", the line under a copy; `undefined` while it is not known. */
export function readersLine(t: Translate, readers: Readers): string {
  if (readers.everyone) {
    return t("saveToPage.readers.everyone");
  }
  if (readers.names.length === 0 && readers.others === 0) {
    return t("saveToPage.readers.only");
  }
  const total = readers.names.length + readers.others;
  if (readers.names.length === 0) {
    return t("saveToPage.readers.count", { count: total });
  }
  const pieces = [
    ...readers.names,
    ...(readers.others > 0 ? [t("saveToPage.readers.others", { count: readers.others })] : []),
  ];
  return t("saveToPage.readers.named", {
    names: joinNames(pieces, t("saveToPage.readers.and")),
    count: total,
  });
}

/** What a refusal says, by name. */
export function refusalText(t: Translate, refusal: Refusal): string {
  switch (refusal.reason) {
    case "scope": {
      return t("saveToPage.refused.scope");
    }
    case "changed": {
      return t("saveToPage.refused.changed");
    }
    case "tooLarge": {
      return refusal.name === undefined
        ? t("saveToPage.refused.tooLargeSome")
        : t("saveToPage.refused.tooLarge", { name: refusal.name });
    }
    case "failed": {
      return t("saveToPage.refused.failed");
    }
  }
}
