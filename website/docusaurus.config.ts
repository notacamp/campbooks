import { themes as prismThemes } from "prism-react-renderer";
import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";

const config: Config = {
  title: "Campbooks",
  tagline: "Your paperwork, sorted.",
  favicon: "img/campbooks-mark.svg",

  future: { v4: true },

  url: "https://campbooks.not-a-camp.com",
  baseUrl: "/",

  organizationName: "notacamp",
  projectName: "campbooks",

  onBrokenLinks: "throw",

  i18n: {
    defaultLocale: "en",
    locales: ["en", "pt", "es", "fr"],
    localeConfigs: {
      en: { label: "English", htmlLang: "en" },
      pt: { label: "Português", htmlLang: "pt-PT" },
      es: { label: "Español", htmlLang: "es" },
      fr: { label: "Français", htmlLang: "fr" },
    },
  },

  // Fonts (Inter, JetBrains Mono, Clash Display) are self-hosted via @font-face in
  // src/css/custom.css — no Google Fonts / Fontshare request, so no visitor IP
  // leaves to a third-party CDN (GDPR). Font files live in static/fonts/.

  presets: [
    [
      "classic",
      {
        docs: {
          sidebarPath: "./sidebars.ts",
          editUrl: "https://github.com/notacamp/campbooks/edit/main/website/",
          showLastUpdateTime: true,
          breadcrumbs: true,
        },
        blog: {
          showReadingTime: true,
          feedOptions: { type: ["rss", "atom"], xslt: true },
          editUrl: "https://github.com/notacamp/campbooks/edit/main/website/",
          onInlineAuthors: "ignore",
          onInlineTags: "ignore",
        },
        pages: {},
        theme: {
          customCss: "./src/css/custom.css",
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: "img/og-image.png",
    colorMode: {
      defaultMode: "light",
      disableSwitch: true,
      respectPrefersColorScheme: false,
    },
    navbar: {
      title: "Campbooks",
      logo: { alt: "Campbooks", src: "img/campbooks-mark.svg" },
      items: [
        {
          type: "docSidebar",
          sidebarId: "docsSidebar",
          position: "left",
          label: "Docs",
        },
        { to: "/blog", label: "Blog", position: "left" },
        { to: "/pricing", label: "Pricing", position: "left" },
        { to: "/about", label: "About", position: "left" },
        {
          href: "https://app.campbooks.not-a-camp.com",
          label: "Try the cloud version",
          position: "right",
          className: "navbar-app-link",
        },
        {
          href: "https://github.com/notacamp/campbooks",
          label: "GitHub",
          position: "right",
        },
        { type: "localeDropdown", position: "right" },
      ],
    },
    footer: {
      style: "light",
      links: [
        {
          title: "Product",
          items: [
            { label: "Documentation", to: "/docs/getting-started/overview" },
            { label: "Pricing", to: "/pricing" },
            { label: "Blog", to: "/blog" },
          ],
        },
        {
          title: "Project",
          items: [
            { label: "About", to: "/about" },
            { label: "GitHub", href: "https://github.com/notacamp/campbooks" },
            { label: "Self-hosting", to: "/self-hosting" },
          ],
        },
        {
          title: "Docs",
          items: [
            { label: "Installation", to: "/docs/getting-started/installation" },
            { label: "Getting started", to: "/docs/getting-started/overview" },
            { label: "Deployment", to: "/docs/deployment/overview" },
          ],
        },
        {
          title: "App",
          items: [
            { label: "Log in", href: "https://app.campbooks.not-a-camp.com" },
            { label: "Made by Not A Camp", href: "https://not-a-camp.com" },
          ],
        },
        {
          title: "Help",
          items: [
            { label: "Support", to: "/support" },
          ],
        },
        {
          title: "Legal",
          items: [
            { label: "Privacy Policy", to: "/privacy" },
            { label: "Terms of Service", to: "/terms" },
            { label: "Delete your data", to: "/data-deletion" },
          ],
        },
      ],
      copyright: `Campbooks — your paperwork, sorted. Source-available under the Sustainable Use License.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.github,
    },
    docs: {
      sidebar: {
        hideable: false,
      },
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
