import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { Button, ChoiceCards, Progress } from "@notacamp/ui";
import { Line } from "../Queue/Queue";

import { Offer } from "./Offer";

const meta = {
  title: "Components/Offer",
  component: Offer,
  args: {
    title: "Move all 1,000 like this to Updates, and new ones too?",
    detail: "None of them looks suspicious.",
    actions: (
      <>
        <Button variant="primary" shortcut="item.choice1">
          Move all 1,000
        </Button>
        <Button variant="ghost" shortcut="overlay.close">
          Just this one
        </Button>
      </>
    ),
    foot: "It becomes a rule. You can undo it, edit it or remove it.",
  },
} satisfies Meta<typeof Offer>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Right after moving one email: which mail, a few of them, and one tap. */
export const Asking: Story = {
  args: {
    children: (
      <>
        <ChoiceCards
          label="Which mail"
          hideLabel
          value="sender"
          onChange={fn()}
          options={[
            {
              value: "sender",
              title: "Everything from notifications@github.com",
              description: "1,000",
            },
            { value: "list", title: "Only the acme/web list", description: "612" },
          ]}
        />
        <div>
          <Line trailing="09:20">[acme/web] New issue: Dark mode flickers (#4185)</Line>
          <Line trailing="08:57">[acme/api] Review requested on #912</Line>
        </div>
      </>
    ),
  },
};

/** While the rule moves them. */
export const Working: Story = {
  args: {
    visor: "thinking",
    title: "Moving 1,000 to Updates",
    detail: "You can keep working. Gmail catches up in the background.",
    children: (
      <Progress label="Moving 1,000 to Updates" value={340} max={1000}>
        340 of 1,000 done
      </Progress>
    ),
    actions: (
      <Button variant="secondary" shortcut="undo">
        Undo all
      </Button>
    ),
    foot: undefined,
  },
};

/** Done, with what was left alone. */
export const Done: Story = {
  args: {
    visor: "happy",
    title: "Moved 998 to Updates. New ones will go there too.",
    detail: "Left 2 alone: they look suspicious.",
    actions: (
      <Button variant="primary" shortcut="undo">
        Undo all
      </Button>
    ),
    foot: undefined,
  },
};
