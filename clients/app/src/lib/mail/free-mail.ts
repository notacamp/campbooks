// Free mail providers (shared by every module that offers a rule for a
// whole domain): a domain of theirs on its own would match strangers. The
// server keeps the full list and has the last word (`invalid_rule`).

/** Free mail providers: a rule for a whole domain of theirs would match strangers (as the server says). */
const FREE_MAIL = new Set([
  "gmail.com",
  "googlemail.com",
  "outlook.com",
  "hotmail.com",
  "live.com",
  "msn.com",
  "yahoo.com",
  "ymail.com",
  "icloud.com",
  "me.com",
  "mac.com",
  "aol.com",
  "proton.me",
  "protonmail.com",
  "gmx.com",
  "gmx.net",
  "zoho.com",
  "sapo.pt",
  "hotmail.fr",
  "orange.fr",
  "yahoo.es",
  "mail.com",
]);

/** Whether `domain` belongs to a free mail provider. */
export function isFreeMailDomain(domain: string): boolean {
  return FREE_MAIL.has(domain.trim().toLowerCase());
}
