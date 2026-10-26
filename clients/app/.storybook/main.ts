import type { StorybookConfig } from "@storybook/react-vite";

// Storybook for the design system (`pnpm storybook`). It reuses vite.config.ts
// (the `~` and `@notacamp/ui` aliases, React and Tailwind), so stories render
// exactly like the app. @notacamp/ui's stories (clients/ui) are shown here too.
const config: StorybookConfig = {
  stories: [
    "../../ui/src/**/*.stories.tsx",
    "../src/**/*.stories.tsx",
    "../stories/**/*.stories.tsx",
  ],
  addons: ["@storybook/addon-a11y", "@storybook/addon-vitest"],
  framework: { name: "@storybook/react-vite", options: {} },
  core: { disableTelemetry: true },
};

export default config;
