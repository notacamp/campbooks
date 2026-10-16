import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { Visor } from "../Visor/Visor";

import { Button, ButtonLink } from "./Button";

const meta = {
  title: "Components/Button",
  component: Button,
  args: { children: "Open proposal", onClick: fn() },
  argTypes: {
    variant: {
      control: "inline-radio",
      options: ["primary", "secondary", "ghost", "danger", "link"],
    },
    size: { control: "inline-radio", options: ["md", "lg"] },
  },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: { variant: "primary", shortcut: "list.open" },
  play: async ({ canvasElement, args }) => {
    await userEvent.click(within(canvasElement).getByRole("button", { name: /Open proposal/ }));
    await expect(args.onClick).toHaveBeenCalledOnce();
  },
};

export const Secondary: Story = {
  args: { variant: "secondary", children: "Reply", shortcut: "item.reply" },
};
export const Ghost: Story = {
  args: { variant: "ghost", children: "Snooze…", shortcut: "item.snooze" },
};

/** An action that runs the AI: the small visor before its label. */
export const Ai: Story = {
  args: { variant: "secondary", ai: true, children: "Draft a reply", keys: "2" },
  play: async ({ canvasElement }) => {
    const button = within(canvasElement).getByRole("button", { name: /Draft a reply/ });
    await expect(button).toHaveAttribute("data-ai", "true");
    await expect(button.querySelector("[data-state]")).not.toBeNull();
  },
};
export const Danger: Story = { args: { variant: "danger", children: "It was not me", keys: "2" } };
export const Link: Story = { args: { variant: "link", children: "Why?" } };

/** 44px, for touch: phone action bars. */
export const Large: Story = {
  args: { variant: "primary", size: "lg", icon: "reply", children: "Reply" },
};

export const WithVisor: Story = {
  args: {
    variant: "ghost",
    leading: <Visor />,
    shortcut: "compose.draft",
    children: "Draft it for me",
  },
};

export const Disabled: Story = { args: { variant: "primary", disabled: true, children: "Send" } };

export const AsLink: StoryObj<typeof ButtonLink> = {
  render: () => (
    <ButtonLink variant="ghost" size="lg" icon="chevron-left" href="#home">
      Home
    </ButtonLink>
  ),
};

/** Every variant at both sizes, each with its keyboard hint. */
export const AllVariants: Story = {
  render: () => (
    <div className="flex flex-col gap-3">
      {(["md", "lg"] as const).map((size) => (
        <div key={size} className="flex flex-wrap items-center gap-2">
          <Button variant="primary" size={size} shortcut="list.open">
            Open
          </Button>
          <Button variant="secondary" size={size} shortcut="item.hold">
            Hold time
          </Button>
          <Button variant="ghost" size={size} shortcut="item.done">
            Done
          </Button>
          <Button variant="danger" size={size} keys="2">
            It was not me
          </Button>
          <Button variant="link" size={size}>
            Undo
          </Button>
        </div>
      ))}
    </div>
  ),
};
