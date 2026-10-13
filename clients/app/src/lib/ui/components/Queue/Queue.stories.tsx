import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { Avatar } from "../Avatar/Avatar";
import { Button } from "../Button/Button";
import { Dot } from "../Dot/Dot";
import { Item } from "../Item/Item";
import { Visor } from "../Visor/Visor";

import {
  AsideSection,
  Figure,
  FigureRow,
  HomeLayout,
  Line,
  LineMeta,
  PageHero,
  QueueGroup,
} from "./Queue";

const meta = {
  title: "Components/Queue",
  component: QueueGroup,
  args: { signal: "reply", name: "Reply", count: 2, children: null },
} satisfies Meta<typeof QueueGroup>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Group: Story = {
  render: (args) => (
    <QueueGroup {...args}>
      <Item
        who="Sofia Pessoa · Brightloop"
        what="Waiting on your review of the Q4 proposal"
        avatar={{ name: "Sofia Pessoa" }}
        due="Thu"
        selected
      />
      <Item
        who="Joana Reis · Atelier Norte"
        what="Needs the signed NDA back before the kickoff on Friday"
        avatar={{ name: "Joana Reis" }}
        due="Fri"
      />
    </QueueGroup>
  ),
};

export const Hero: Story = {
  render: () => (
    <PageHero
      visor="alert"
      dateLine="Monday 12 October"
      title="9 things need you"
      subline="48 messages sorted since Friday. Campbooks handled the rest."
    />
  ),
};

export const HeroCompact: Story = {
  render: () => <PageHero compact title="5 things need you" subline="48 sorted since Friday" />,
};

export const Aside: Story = {
  render: () => (
    <div className="flex max-w-aside flex-col gap-6">
      <AsideSection title="Today">
        <Line time="09:00" leading={<Dot signal="do" />} trailing={<LineMeta>Focus</LineMeta>}>
          Held: Q4 review
        </Line>
        <Line time="10:00" leading={<Dot signal="reply" />} trailing={<LineMeta>Video</LineMeta>}>
          Call with Brightloop
        </Line>
      </AsideSection>
      <AsideSection title="Money">
        <FigureRow>
          <Figure value="€460.40" label="2 bills due this week" />
          <Figure value="€2,340.00" label="Brightloop INV-0042, due 20 Oct" />
        </FigureRow>
        <Line leading={<Dot signal="pay" />}>Statement of 8 Oct: 31 matched, 3 to check</Line>
      </AsideSection>
      <AsideSection title="Waiting on others">
        <Line
          leading={<Avatar name="Inês Carvalho" size="sm" />}
          trailing={<Button variant="ghost">Nudge</Button>}
        >
          Inês Carvalho: logo SVGs, 8 days
        </Line>
      </AsideSection>
      <AsideSection title="What Campbooks did" leading={<Visor />}>
        <Line
          trailing={
            <Button variant="link" onClick={fn()}>
              Undo
            </Button>
          }
        >
          Filed 4 receipts in Documents
        </Line>
        <Line
          undone
          trailing={
            <Button variant="link" onClick={fn()}>
              Redo
            </Button>
          }
        >
          Moved 2 newsletters out of Primary
        </Line>
      </AsideSection>
    </div>
  ),
};

export const Layout: Story = {
  parameters: { layout: "fullscreen" },
  render: () => (
    <HomeLayout
      asideLabel="Context"
      aside={
        <AsideSection title="Today">
          <Line time="09:00">Held: Q4 review</Line>
        </AsideSection>
      }
    >
      <PageHero title="1 thing needs you" dateLine="Monday 12 October" />
      <QueueGroup signal="pay" name="Pay" count={1}>
        <Item
          who="Cloudhost"
          what="Invoice CH-20418 for October hosting"
          avatar={{ name: "Cloudhost", kind: "service" }}
          due="Fri · €148.00"
        />
      </QueueGroup>
    </HomeLayout>
  ),
};
