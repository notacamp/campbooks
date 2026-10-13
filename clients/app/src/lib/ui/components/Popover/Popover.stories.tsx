import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "../Button/Button";
import { Select } from "../Field/Field";
import { Segmented } from "../Segmented/Segmented";
import { AccountButton } from "../Shell/Shell";

import { Popover, PopoverSection } from "./Popover";

function AccountMenu({ placement }: { placement: "top-start" | "bottom-end" }) {
  const [theme, setTheme] = useState<"system" | "light" | "dark">("system");
  const [language, setLanguage] = useState<"en" | "pt-PT">("en");
  return (
    <Popover
      label="Account"
      placement={placement}
      renderTrigger={(props) =>
        placement === "top-start" ? (
          <AccountButton {...props} name="Ana Duarte" email="ana@example.com" />
        ) : (
          <AccountButton {...props} compact label="Account" name="Ana Duarte" />
        )
      }
    >
      <PopoverSection>
        <strong>Ana Duarte</strong>
        <span>ana@example.com</span>
      </PopoverSection>
      <PopoverSection title="Theme">
        <Segmented
          label="Theme"
          value={theme}
          onChange={setTheme}
          options={[
            { value: "system", label: "System" },
            { value: "light", label: "Light" },
            { value: "dark", label: "Dark" },
          ]}
        />
      </PopoverSection>
      <PopoverSection>
        <Select
          label="Language"
          value={language}
          onChange={setLanguage}
          options={[
            { value: "en", label: "English" },
            { value: "pt-PT", label: "Português" },
          ]}
        />
      </PopoverSection>
      <PopoverSection>
        <Button variant="ghost" icon="sign-out" onClick={fn()}>
          Sign out
        </Button>
      </PopoverSection>
    </Popover>
  );
}

const meta = {
  title: "Components/Popover",
  parameters: { layout: "centered" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** The sidebar's account menu opens upwards from the bottom of the sidebar. */
export const SidebarAccount: Story = {
  render: () => (
    <div className="flex h-hit w-sidebar items-end">
      <AccountMenu placement="top-start" />
    </div>
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: /Ana Duarte/u }));
    await expect(canvas.getByRole("dialog", { name: "Account" })).toBeVisible();
  },
};

/** On phones it hangs from the avatar in the header. */
export const PhoneAccount: Story = {
  render: () => <AccountMenu placement="bottom-end" />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Account" }));
    await expect(canvas.getByRole("dialog", { name: "Account" })).toBeVisible();
    await userEvent.keyboard("{Escape}");
    await expect(canvas.getByRole("button", { name: "Account" })).toHaveFocus();
  },
};
