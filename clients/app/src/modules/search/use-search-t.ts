import { useTranslation } from "react-i18next";

import type { Translate } from "./model";

/** The `search` namespace's `t` for keys only known at runtime (filter operators, codes). */
export function useSearchT(): { t: Translate; language: string } {
  const { t, i18n } = useTranslation("search");
  return { t: t as unknown as Translate, language: i18n.language };
}
