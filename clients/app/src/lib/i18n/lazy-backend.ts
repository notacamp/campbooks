import type { BackendModule } from "i18next";

export type Messages = Record<string, unknown>;
/** Loads one namespace's messages for one locale (typically a dynamic `import()`). */
export type NamespaceLoader = (locale: string) => Promise<Messages>;

/**
 * Turns the result of `import.meta.glob("…/<namespace>/i18n/<locale>.json")`
 * into one loader per namespace. Each locale file stays its own chunk, so a
 * module's strings are downloaded only when one of its screens renders.
 */
export function loadersFromGlob(
  files: Record<string, () => Promise<Messages>>,
): Record<string, NamespaceLoader> {
  const byNamespace = new Map<string, Map<string, () => Promise<Messages>>>();
  for (const [path, load] of Object.entries(files)) {
    const match = /([^/]+)\/i18n\/([^/]+)\.json$/.exec(path);
    if (match === null) {
      continue;
    }
    const [, namespace = "", locale = ""] = match;
    const locales = byNamespace.get(namespace) ?? new Map<string, () => Promise<Messages>>();
    locales.set(locale, load);
    byNamespace.set(namespace, locales);
  }
  return Object.fromEntries(
    [...byNamespace].map(([namespace, locales]) => [
      namespace,
      (locale: string) => {
        const load = locales.get(locale);
        return load === undefined
          ? Promise.reject(new Error(`No "${namespace}" messages for locale "${locale}"`))
          : load();
      },
    ]),
  );
}

/** An i18next backend that resolves namespaces through {@link NamespaceLoader}s. */
export function lazyBackend(loaders: Record<string, NamespaceLoader>): BackendModule {
  return {
    type: "backend",
    init: () => {
      // Nothing to configure: the loaders are captured above.
    },
    read: (language, namespace, callback) => {
      const load = loaders[namespace];
      if (load === undefined) {
        callback(new Error(`Unknown i18n namespace "${namespace}"`), false);
        return;
      }
      load(language).then(
        (messages) => {
          callback(null, messages);
        },
        (error: unknown) => {
          callback(
            new Error(`Failed to load i18n namespace "${namespace}" (${language})`, {
              cause: error,
            }),
            false,
          );
        },
      );
    },
  };
}
