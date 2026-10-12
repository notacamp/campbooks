import { createI18n, detectLocale, loadersFromGlob, type Messages } from "~/lib/i18n";
import { readPref } from "~/lib/storage/prefs";

import en from "./i18n/en.json";
import es from "./i18n/es.json";
import fr from "./i18n/fr.json";
import pt from "./i18n/pt.json";

// Every module's `i18n/<locale>.json` becomes a lazily loaded namespace named
// after the module folder (`modules/system/i18n/pt.json` → namespace "system").
const moduleMessages = import.meta.glob<Messages>("../modules/*/i18n/*.json", {
  import: "default",
});

export const i18n = createI18n({
  locale: detectLocale(readPref("locale"), navigator.languages),
  shell: { en, pt, es, fr },
  loaders: loadersFromGlob(moduleMessages),
});

const syncDocumentLanguage = (language: string) => {
  document.documentElement.lang = language;
};
syncDocumentLanguage(i18n.language);
i18n.on("languageChanged", syncDocumentLanguage);
