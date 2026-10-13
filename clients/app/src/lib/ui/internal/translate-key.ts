import { useTranslation } from "react-i18next";

/**
 * Translates a key that is only known at runtime, written with its
 * namespace ("shortcuts:nav.inbox"). Shortcut labels are such keys: modules
 * register them, so they can't be checked against the typed resources.
 */
export function useTranslateKey(): (key: string) => string {
  const { t } = useTranslation();
  const untyped = t as unknown as (key: string) => string;
  return (key) => untyped(key);
}
