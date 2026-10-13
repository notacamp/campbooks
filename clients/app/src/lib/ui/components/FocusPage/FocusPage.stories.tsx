import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { Button } from "../Button/Button";
import { TextField } from "../Field/Field";
import { Visor } from "../Visor/Visor";

import { Brand, FocusPage } from "./FocusPage";

const meta = {
  title: "Components/FocusPage",
  component: FocusPage,
  parameters: { layout: "fullscreen" },
  args: { title: "Your inbox, sorted for you" },
} satisfies Meta<typeof FocusPage>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Signed out: the brand, one line, one action. */
export const Landing: Story = {
  args: {
    top: <Brand name="Campbooks" />,
    leading: <Visor size="xl" />,
    lead: "Campbooks reads your mail and brings you only what needs you.",
    actions: (
      <Button variant="primary" size="lg">
        Sign in
      </Button>
    ),
    footer: "Source available. Your data stays in your workspace.",
  },
};

/** One question per view, as a form: Enter submits. */
export const Question: Story = {
  args: {
    top: <Brand name="Campbooks" />,
    eyebrow: "Step 1 of 2",
    title: "What should we call your workspace?",
    lead: "Most people use their company’s name, or “Household”.",
    onSubmit: fn(),
    children: (
      <TextField label="Workspace name" hideLabel size="lg" placeholder="Duarte & Pessoa" />
    ),
    actions: (
      <>
        <Button type="submit" variant="primary" size="lg">
          Continue
        </Button>
        <Button variant="ghost" size="lg">
          Skip
        </Button>
      </>
    ),
  },
};

/** The page can preview a workspace accent without changing the app's. */
export const PersonalAccent: Story = {
  args: {
    ...Question.args,
    accent: "personal",
    eyebrow: "Step 2 of 2",
    title: "What is it for?",
  },
};
