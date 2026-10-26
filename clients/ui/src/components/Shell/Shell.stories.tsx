import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { ActionBar } from "../ActionBar/ActionBar";
import { Button } from "../Button/Button";
import { ChipLink, ChipRow } from "../Chip/Chip";
import { IconButton } from "../IconButton/IconButton";
import { NavItem } from "../NavItem/NavItem";
import { Segmented } from "../Segmented/Segmented";
import { Heading } from "../Typography/Typography";

import {
  AccountButton,
  ListPane,
  PaneHeader,
  ReaderBody,
  ReaderFooter,
  ReaderPane,
  SearchButton,
  Shell,
  Sidebar,
  SidebarFooter,
  PhoneHeader,
  PhoneSubnav,
  Spacer,
  StatusLine,
  WorkspaceMark,
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
          <Button variant="ghost" shortcut="item.snooze">
            Snooze…
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

/** Who is signed in: the sidebar's account button and the phone's avatar. */
export const Account: Story = {
  args: { children: null },
  render: () => (
    <div className="flex w-sidebar flex-col gap-2">
      <AccountButton name="Ana Duarte" email="ana@example.com" onClick={fn()} />
      <AccountButton compact label="Account" name="Ana Duarte" onClick={fn()} />
    </div>
  ),
};

/** Each workspace's mark carries its own accent. */
export const Marks: Story = {
  args: { children: null },
  render: () => (
    <div className="flex gap-2">
      <WorkspaceMark mark="DP" accent="business" />
      <WorkspaceMark mark="HO" accent="personal" />
      <WorkspaceMark mark="QV" accent="forest" />
      <WorkspaceMark mark="HO" accent="personal" size="lg" />
    </div>
  ),
};

/** Below 48rem the sidebar gives way to this header (view it at a phone width). */
export const Phone: Story = {
  args: { children: null },
  render: () => (
    <Shell>
      <PhoneHeader>
        <WorkspaceSwitcher
          compact
          mark="DP"
          name="Duarte & Pessoa"
          kind="Business"
          onClick={fn()}
        />
        <Spacer />
        <AccountButton compact label="Account" name="Ana Duarte" onClick={fn()} />
      </PhoneHeader>
    </Shell>
  ),
};

/** A page's sections under the phone header (Settings), as chips. */
export const PhoneSections: Story = {
  args: { children: null },
  render: () => (
    <Shell>
      <PhoneHeader>
        <WorkspaceSwitcher
          compact
          mark="DP"
          name="Duarte & Pessoa"
          kind="Business"
          onClick={fn()}
        />
        <Spacer />
        <AccountButton compact label="Account" name="Ana Duarte" onClick={fn()} />
      </PhoneHeader>
      <PhoneSubnav label="Settings">
        <ChipRow label="Settings sections">
          <ChipLink href="#members">Members</ChipLink>
          <ChipLink href="#mailboxes" current>
            Mailboxes
          </ChipLink>
          <ChipLink href="#sign-in">Ways to sign in</ChipLink>
        </ChipRow>
      </PhoneSubnav>
    </Shell>
  ),
};

/**
 * A reader's footer, pinned while the thread scrolls under it: the way to
 * reply from 48rem, the floating bar below (view it at a phone width).
 */
export const ReaderWithFooter: Story = {
  args: { children: null },
  render: () => (
    <Shell>
      <ReaderPane label="Conversation">
        <ReaderBody>
          <Heading level={2} size="page">
            Kitchen shelving quote
          </Heading>
          {Array.from({ length: 12 }, (_, index) => (
            <p key={index}>Perfect, approved. When can you start?</p>
          ))}
        </ReaderBody>
        <ReaderFooter
          prompt={
            <Button icon="reply" shortcut="item.reply" grow>
              Reply to Marta Silva
            </Button>
          }
          phoneBar={
            <ActionBar label="Conversation actions" phoneOnly aboveTabBar>
              <Button variant="primary" size="lg" icon="reply" grow>
                Reply
              </Button>
              <Button size="lg" icon="archive" grow>
                Done
              </Button>
            </ActionBar>
          }
        />
      </ReaderPane>
    </Shell>
  ),
};

/** With the composer open, it takes the footer and grows up to its limit. */
export const ReaderWithComposer: Story = {
  args: { children: null },
  render: () => (
    <Shell>
      <ReaderPane label="Conversation">
        <ReaderBody>
          <p>Perfect, approved. When can you start?</p>
        </ReaderBody>
        <ReaderFooter
          slot={
            <section aria-label="Reply to Marta Silva" className="flex flex-col gap-2 p-3">
              <textarea aria-label="Your reply" rows={4} defaultValue="Monday works." />
              {Array.from({ length: 30 }, (_, index) => (
                <p key={index}>Monday works. I will bring the brackets.</p>
              ))}
            </section>
          }
          prompt={<Button grow>Reply to Marta Silva</Button>}
        />
      </ReaderPane>
    </Shell>
  ),
};
