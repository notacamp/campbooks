import { describe, expect, it } from "vitest";

import { createI18n, detectLocale, loadersFromGlob } from "./index";
import { lazyBackend } from "./lazy-backend";

// Keys must exist in the typed resources (src/app/i18next.d.ts); values are test data.
const shell = {
  en: { loading: "Loading" },
  pt: { loading: "A carregar" },
  es: { loading: "Cargando" },
  fr: { loading: "Chargement" },
};

describe("detectLocale", () => {
  it("prefers a saved choice", () => {
    expect(detectLocale("fr", ["pt-PT"])).toBe("fr");
  });

  it("ignores an unsupported saved choice and uses the browser language", () => {
    expect(detectLocale("de", ["pt-PT", "en"])).toBe("pt");
  });

  it("falls back to English", () => {
    expect(detectLocale(null, ["de-DE", ""])).toBe("en");
  });
});

describe("loadersFromGlob", () => {
  const loaders = loadersFromGlob({
    "../modules/mail/i18n/en.json": () => Promise.resolve({ inbox: "Inbox" }),
    "../modules/mail/i18n/pt.json": () => Promise.resolve({ inbox: "Caixa de entrada" }),
    "../modules/mail/README.md": () => Promise.resolve({}),
  });

  it("groups files by namespace and locale", async () => {
    expect(Object.keys(loaders)).toStrictEqual(["mail"]);
    await expect(loaders["mail"]?.("pt")).resolves.toStrictEqual({ inbox: "Caixa de entrada" });
  });

  it("rejects a locale the namespace does not ship", async () => {
    await expect(loaders["mail"]?.("fr")).rejects.toThrow('No "mail" messages for locale "fr"');
  });
});

describe("lazyBackend", () => {
  function read(backend: ReturnType<typeof lazyBackend>, namespace: string) {
    return new Promise<{ error: unknown; data: unknown }>((resolve) => {
      backend.read("en", namespace, (error, data) => {
        resolve({ error, data });
      });
    });
  }

  it("reports unknown namespaces", async () => {
    const { error } = await read(lazyBackend({}), "nope");

    expect(error).toStrictEqual(new Error('Unknown i18n namespace "nope"'));
  });

  it("reports loader failures with their cause", async () => {
    const offline = new Error("offline");
    const backend = lazyBackend({ boom: () => Promise.reject(offline) });

    const { error } = await read(backend, "boom");

    expect(error).toStrictEqual(new Error('Failed to load i18n namespace "boom" (en)'));
    expect((error as Error).cause).toBe(offline);
  });
});

describe("createI18n", () => {
  it("serves the shell synchronously and loads module namespaces on demand", async () => {
    let loads = 0;
    const i18n = createI18n({
      locale: "pt",
      shell,
      loaders: {
        system: (locale) => {
          loads += 1;
          return Promise.resolve({
            title: locale === "pt" ? "Estado do sistema" : "System status",
          });
        },
      },
    });

    expect(i18n.t("loading")).toBe("A carregar");
    expect(i18n.hasLoadedNamespace("system")).toBe(false);

    await i18n.loadNamespaces("system");

    expect(i18n.t("title", { ns: "system" })).toBe("Estado do sistema");
    expect(loads).toBeGreaterThan(0);
  });
});
