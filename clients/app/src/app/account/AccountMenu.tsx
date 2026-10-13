import { useState, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { signOutAndLeave, useSession } from "~/lib/api";
import { SUPPORTED_LOCALES, type Locale } from "~/lib/i18n";
import { writePref } from "~/lib/storage/prefs";
import {
  AccountButton,
  Button,
  ButtonLink,
  DENSITIES,
  Popover,
  PopoverSection,
  Segmented,
  Select,
  Text,
  THEME_MODES,
  useTheme,
} from "~/lib/ui";

/** Each language in its own name: the same in every locale. */
const LANGUAGE_NAMES: Record<Locale, string> = {
  en: "English",
  "pt-PT": "Português (Portugal)",
  "es-ES": "Español (España)",
  fr: "Français",
};

export interface AccountMenuProps {
  /** Just the avatar, hanging from the phone header. */
  compact?: boolean | undefined;
  /** Settings → Ways to sign in, as an in-app link. */
  waysIn?: { href: string; onClick: (event: MouseEvent<HTMLElement>) => void } | undefined;
  /** Settings → Apps and integrations, as an in-app link. */
  apps?: { href: string; onClick: (event: MouseEvent<HTMLElement>) => void } | undefined;
}

/**
 * Who is signed in, their theme, density and language, how they sign in, the
 * apps they let in, and Sign out.
 */
export function AccountMenu({ compact = false, waysIn, apps }: AccountMenuProps) {
  const { t, i18n } = useTranslation();
  const session = useSession();
  const theme = useTheme();
  const [leaving, setLeaving] = useState(false);
  const user = session.data?.user;
  const email = user?.email ?? undefined;
  const name = user?.name ?? email ?? t("account.label");
  const language = (SUPPORTED_LOCALES as readonly string[]).includes(i18n.language)
    ? (i18n.language as Locale)
    : "en";

  return (
    <Popover
      label={t("account.label")}
      placement={compact ? "bottom-end" : "top-start"}
      renderTrigger={(props) => (
        <AccountButton
          {...props}
          compact={compact}
          label={t("account.label")}
          name={name}
          email={email}
        />
      )}
    >
      {(close) => (
        <>
          <PopoverSection>
            <Text weight="semibold" truncate>
              {name}
            </Text>
            {email === undefined || email === name ? null : (
              <Text tone="tertiary" size="sm" truncate>
                {email}
              </Text>
            )}
          </PopoverSection>
          <PopoverSection title={t("account.theme")}>
            <Segmented
              label={t("account.theme")}
              value={theme.theme}
              onChange={theme.setTheme}
              options={THEME_MODES.map((mode) => ({
                value: mode,
                label: t(`account.themes.${mode}`),
              }))}
            />
          </PopoverSection>
          <PopoverSection title={t("account.density")}>
            <Segmented
              label={t("account.density")}
              value={theme.density}
              onChange={theme.setDensity}
              options={DENSITIES.map((density) => ({
                value: density,
                label: t(`account.densities.${density}`),
              }))}
            />
          </PopoverSection>
          <PopoverSection>
            <Select
              label={t("account.language")}
              value={language}
              options={SUPPORTED_LOCALES.map((locale) => ({
                value: locale,
                label: LANGUAGE_NAMES[locale],
              }))}
              onChange={(locale) => {
                // Device-only for now. TODO(#69): also save it to the user's
                // profile on the server so emails and other devices follow.
                writePref("locale", locale);
                void i18n.changeLanguage(locale);
              }}
            />
          </PopoverSection>
          {waysIn === undefined ? null : (
            <PopoverSection>
              <ButtonLink
                variant="ghost"
                icon="key"
                href={waysIn.href}
                onClick={(event) => {
                  waysIn.onClick(event);
                  close();
                }}
              >
                {t("account.waysIn")}
              </ButtonLink>
            </PopoverSection>
          )}
          {apps === undefined ? null : (
            <PopoverSection>
              <ButtonLink
                variant="ghost"
                icon="settings"
                href={apps.href}
                onClick={(event) => {
                  apps.onClick(event);
                  close();
                }}
              >
                {t("account.apps")}
              </ButtonLink>
            </PopoverSection>
          )}
          <PopoverSection>
            <Button
              variant="ghost"
              icon="sign-out"
              disabled={leaving}
              onClick={() => {
                setLeaving(true);
                signOutAndLeave().catch(() => {
                  setLeaving(false);
                });
              }}
            >
              {leaving ? t("account.signingOut") : t("account.signOut")}
            </Button>
          </PopoverSection>
        </>
      )}
    </Popover>
  );
}
