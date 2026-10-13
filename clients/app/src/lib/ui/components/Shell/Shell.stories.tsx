import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { Button } from "../Button/Button";
import { IconButton } from "../IconButton/IconButton";
import { NavItem } from "../NavItem/NavItem";
import { Segmented } from "../Segmented/Segmented";
import { Heading } from "../Typography/Typography";

import {
  ListPane,
  PaneHeader,
  ReaderPane,
  SearchButton,
  Shell,
  Sidebar,
  SidebarFooter,
  Spacer,
  StatusLine,
  WorkspaceSwitcher,
} from "./Shell";

const meta = {
  title: "Components/Shell",
  component: Shell,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Shell>;

export default meta;
type Story = StoryObj<typeof meta>;

const sidebar = (
  <Sidebar>
    <WorkspaceSwitcher mark="DP" name="Duarte & Pessoa" kind="Business" onClick={fn()} />
    <SearchButton onClick={fn()} />
    <NavItem label="Home" icon="home" count={6} shortcut="nav.home" current />
    <NavItem label="Inbox" icon="inbox" count={7} shortcut="nav.inbox" />
    <NavItem label="Primary" count={12} sub />
    <NavItem label="Updates" count={31} sub />
    <NavItem label="Schedule" icon="schedule" shortcut="nav.schedule" />
    <NavItem label="Tasks" icon="check" count={7} shortcut="nav.tasks" />
    <NavItem label="Documents" icon="document" shortcut="nav.documents" />
    <NavItem label="Money" icon="money" shortcut="nav.money" />
    <SidebarFooter>
      <StatusLine visor="reading" title="Reading 3 new messages">
        48 sorted since Friday
      </StatusLine>
      <Button variant="ghost" icon="keyboard" shortcut="sheet.open">
        Keyboard shortcuts
      </Button>
    </SidebarFooter>
  </Sidebar>
);

/** Sidebar, list pane and reader. Below 48rem the sidebar gives way to the tab bar. */
export const Layout: Story = {
  args: { children: null },
  render: () => (
    <Shell>
      {sidebar}
      <ListPane label="Conversations">
        <PaneHeader>
          <Heading level={1} size="pane">
            Primary
          </Heading>
          <Spacer />
          <Segmented
            label="Show"
            value="all"
            onChange={fn()}
            options={[
              { value: "all", label: "All" },
              { value: "unread", label: "Unread" },
            ]}
          />
        </PaneHeader>
      </ListPane>
      <ReaderPane label="Conversation">
        <PaneHeader>
          <Button shortcut="item.done">Done</Button>
          <Button variant="ghost" shortcut="item.later">
            Later
          </Button>
          <Spacer />
          <IconButton label="Previous conversation" icon="chevron-up" shortcut="list.previous" />
          <IconButton label="Next conversation" icon="chevron-down" shortcut="list.next" />
        </PaneHeader>
      </ReaderPane>
    </Shell>
  ),
};

export const SidebarOnly: Story = {
  args: { children: null },
  render: () => <Shell>{sidebar}</Shell>,
};

export const CompactWorkspace: Story = {
  args: { children: null },
  render: () => (
    <WorkspaceSwitcher compact mark="DP" name="Duarte & Pessoa" kind="Business" onClick={fn()} />
  ),
};

export const Statuses: Story = {
  args: { children: null },
  render: () => (
    <div className="flex flex-col gap-4">
      <StatusLine visor="reading" title="Reading 3 new messages">
        48 sorted since Friday
      </StatusLine>
      <StatusLine visor="asleep" title="Campbooks is paused">
        Mail still arrives in the Inbox, unsorted. Nothing is read by AI.
      </StatusLine>
    </div>
  ),
};
