// Free mail providers (shared by every module that offers a rule for a
// whole domain): a domain of theirs on its own would match strangers. The
// same list as the server's (`campbooks_ai::decide::taught`), which has the
// last word (`invalid_rule`).

/** Free mail providers: a rule for a whole domain of theirs would match strangers (as the server says). */
const FREE_MAIL = new Set([
  "gmail.com",
  "googlemail.com",
  "outlook.com",
  "outlook.pt",
  "outlook.es",
  "outlook.fr",
  "hotmail.com",
  "hotmail.co.uk",
  "hotmail.fr",
  "hotmail.es",
  "hotmail.it",
  "hotmail.de",
  "live.com",
  "live.co.uk",
  "live.fr",
  "msn.com",
  "yahoo.com",
  "yahoo.co.uk",
  "yahoo.fr",
  "yahoo.es",
  "yahoo.de",
  "ymail.com",
  "rocketmail.com",
  "icloud.com",
  "me.com",
  "mac.com",
  "aol.com",
  "aol.co.uk",
  "proton.me",
  "protonmail.com",
  "pm.me",
  "gmx.com",
  "gmx.net",
  "gmx.de",
  "gmx.fr",
  "web.de",
  "t-online.de",
  "yandex.com",
  "yandex.ru",
  "mail.ru",
  "fastmail.com",
  "fastmail.fm",
  "zoho.com",
  "zohomail.com",
  "tutanota.com",
  "tuta.io",
  "sapo.pt",
  "orange.fr",
  "free.fr",
  "laposte.net",
  "mail.com",
  "hey.com",
]);

/** Whether `domain` belongs to a free mail provider. */
export function isFreeMailDomain(domain: string): boolean {
  return FREE_MAIL.has(domain.trim().toLowerCase());
}
