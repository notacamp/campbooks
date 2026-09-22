/**
 * OAuthProviderMarks — inline SVG brand marks for OAuth sign-in providers.
 *
 * Each mark is the canonical, fixed-palette logo for its identity provider.
 * Brand colors MUST match the official guidelines exactly — they cannot be
 * swapped for design-system tokens without misrepresenting the providers.
 *
 * eslint-disable local/no-raw-hex is intentional: these hex values are not
 * design-system color choices — they are trademarked brand colours that are
 * required to be reproduced faithfully by each provider's branding policy.
 */
/* eslint-disable local/no-raw-hex -- brand mark colors; see module docstring */
import { type FC } from "react";

// ── Google "G" mark ───────────────────────────────────────────────────────────

export const GoogleMark: FC = () => (
  <svg
    width="16"
    height="16"
    viewBox="0 0 24 24"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
    focusable="false"
  >
    <path
      d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
      fill="#4285F4"
    />
    <path
      d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
      fill="#34A853"
    />
    <path
      d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l3.66-2.84z"
      fill="#FBBC05"
    />
    <path
      d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
      fill="#EA4335"
    />
  </svg>
);

// ── Zoho mark ─────────────────────────────────────────────────────────────────

export const ZohoMark: FC = () => (
  <svg
    width="16"
    height="16"
    viewBox="0 0 64 64"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
    focusable="false"
  >
    <rect width="64" height="64" rx="8" fill="#E42527" />
    <path d="M10 44 L28 20 L10 20 L10 14 L38 14 L20 38 L38 38 L38 44 Z" fill="#fff" />
    <circle cx="52" cy="28" r="10" fill="#F4A41B" />
    <circle cx="52" cy="28" r="6" fill="#fff" />
    <rect x="44" y="38" width="18" height="12" fill="#2D72B8" />
  </svg>
);

// ── Microsoft mark ────────────────────────────────────────────────────────────

export const MicrosoftMark: FC = () => (
  <svg
    width="16"
    height="16"
    viewBox="0 0 21 21"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
    focusable="false"
  >
    <rect x="1" y="1" width="9" height="9" fill="#F35325" />
    <rect x="11" y="1" width="9" height="9" fill="#81BC06" />
    <rect x="1" y="11" width="9" height="9" fill="#05A6F0" />
    <rect x="11" y="11" width="9" height="9" fill="#FFBA08" />
  </svg>
);
