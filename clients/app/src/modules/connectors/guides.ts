/**
 * The app-password guides of the connect flow (campbooks-cloud#314): which
 * provider's steps to show for an address, and the provider's own public
 * help pages they link to. The links are constants, verified on 2026-10-25;
 * none is ever built from what a server or the member typed.
 */

/** A guide: a provider's steps, or the general one for any other domain. */
type GuideId = "yahoo" | "aol" | "icloud" | "fastmail" | "gmx" | "webde" | "yandex" | "custom";

export interface Guide {
  id: GuideId;
  /** The provider's page on app passwords. */
  help?: string | undefined;
  /** The provider's page on turning IMAP on, when it must be turned on first. */
  imap?: string | undefined;
}

const GUIDES: Record<Exclude<GuideId, "gmx">, Guide> = {
  yahoo: { id: "yahoo", help: "https://help.yahoo.com/kb/SLN15241.html" },
  aol: {
    id: "aol",

    help: "https://help.aol.com/articles/create-and-manage-app-password",
  },
  icloud: { id: "icloud", help: "https://support.apple.com/en-us/102654" },
  fastmail: {
    id: "fastmail",

    help: "https://www.fastmail.help/hc/en-us/articles/360058752854-App-passwords",
  },
  webde: {
    id: "webde",

    help: "https://hilfe.web.de/sicherheit/2fa/anwendungsspezifisches-passwort.html",
    imap: "https://hilfe.web.de/pop-imap/einschalten.html",
  },
  yandex: {
    id: "yandex",

    help: "https://yandex.com/support/id/en/authorization/app-passwords",
    imap: "https://yandex.com/support/yandex-360/customers/mail/en/mail-clients/others",
  },
  custom: { id: "custom" },
};

/** GMX has an international site (gmx.com) and a German one (gmx.net, .de, .at, .ch). */
const GMX_INTERNATIONAL: Guide = {
  id: "gmx",

  help: "https://support.gmx.com/security/2fa/application-specific-passwords.html",
  imap: "https://support.gmx.com/pop-imap/toggle.html",
};
const GMX_GERMAN: Guide = {
  id: "gmx",

  help: "https://hilfe.gmx.net/sicherheit/2fa/anwendungsspezifisches-passwort.html",
  imap: "https://hilfe.gmx.net/pop-imap/einschalten.html",
};

/** The bundled provider database's ids (`provider_hint`) of the providers with a guide. */
const BY_HINT: Readonly<Record<string, GuideId>> = {
  "yahoo.com": "yahoo",
  "aol.com": "aol",
  "me.com": "icloud",
  "icloud.com": "icloud",
  "messagingengine.com": "fastmail",
  "fastmail.com": "fastmail",
  "gmx.com": "gmx",
  "gmx.net": "gmx",
  "web.de": "webde",
  "yandex.ru": "yandex",
  "yandex.com": "yandex",
};

/** The guide of a domain, by its own name, for addresses found another way. */
function byDomain(domain: string): GuideId | undefined {
  const brand = domain.split(".", 1)[0] ?? "";
  if (brand === "yahoo" || domain === "ymail.com" || domain === "rocketmail.com") {
    return "yahoo";
  }
  if (brand === "aol") {
    return "aol";
  }
  if (domain === "icloud.com" || domain === "me.com" || domain === "mac.com") {
    return "icloud";
  }
  if (brand === "fastmail") {
    return "fastmail";
  }
  if (brand === "gmx") {
    return "gmx";
  }
  if (domain === "web.de") {
    return "webde";
  }
  if (brand === "yandex" || domain === "ya.ru") {
    return "yandex";
  }
  return undefined;
}

/**
 * The guide for `address`: by discovery's `provider_hint` first, else by the
 * address's domain, else the general one.
 */
export function guideFor(address: string, providerHint?: string | null): Guide {
  const domain = address.trim().toLowerCase().split("@").at(-1) ?? "";
  const id =
    (providerHint === null || providerHint === undefined ? undefined : BY_HINT[providerHint]) ??
    byDomain(domain) ??
    "custom";
  if (id === "gmx") {
    return domain === "gmx.com" || domain === "gmx.us" || domain === "gmx.co.uk"
      ? GMX_INTERNATIONAL
      : GMX_GERMAN;
  }
  return GUIDES[id];
}

/** Every link a guide may show (tests check they are all the providers' own HTTPS pages). */
export const GUIDE_LINKS: readonly string[] = [
  ...Object.values(GUIDES).flatMap((guide) => [guide.help, guide.imap]),
  ...[GMX_INTERNATIONAL, GMX_GERMAN].flatMap((guide) => [guide.help, guide.imap]),
].filter((link): link is string => link !== undefined);
