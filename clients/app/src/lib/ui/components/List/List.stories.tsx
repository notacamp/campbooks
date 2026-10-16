import type { Meta, StoryObj } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { Avatar } from "../Avatar/Avatar";
import { Button } from "../Button/Button";
import { Select } from "../Field/Field";
import { InlineConfirm } from "../InlineConfirm/InlineConfirm";
import { ProviderMark } from "../ProviderMark/ProviderMark";
import { StatusText } from "../StatusText/StatusText";

import { List, ListRow } from "./List";

const meta = {
  title: "Components/List",
  parameters: { layout: "padded" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const roles = [
  { value: "owner", label: "Owner" },
  { value: "admin", label: "Admin" },
  { value: "member", label: "Member" },
] as const;

/** Members with their role; one row asking to confirm a removal. */
export const Members: Story = {
  render: () => (
    <div className="max-w-measure">
      <List label="Members">
        <ListRow
          leading={<Avatar name="Ana Duarte" />}
          title="Ana Duarte"
          badge="You"
          meta="ana@example.com"
          trailing={<Select hideLabel label="Role" value="owner" onChange={fn()} options={roles} />}
        />
        <ListRow
          leading={<Avatar name="Sofia Pessoa" />}
          title="Sofia Pessoa"
          meta="sofia@example.com"
          trailing={
            <>
              <Select hideLabel label="Role" value="admin" onChange={fn()} options={roles} />
              <Button variant="ghost">Remove</Button>
            </>
          }
        >
          <InlineConfirm
            confirmLabel="Remove"
            cancelLabel="Cancel"
            onConfirm={fn()}
            onCancel={fn()}
          >
            Remove Sofia Pessoa from Duarte &amp; Pessoa? She loses access right away.
          </InlineConfirm>
        </ListRow>
        <ListRow
          leading={<Avatar name="Rui Matos" />}
          title="Maria do Carmo Sant’Ana Albuquerque e Vasconcelos"
          meta="maria.do.carmo.albuquerque.vasconcelos@example.com"
          trailing={<Button variant="ghost">Remove</Button>}
        />
      </List>
    </div>
  ),
};

/** Pending invitations: an expired one is muted. */
export const Invitations: Story = {
  render: () => (
    <div className="max-w-measure">
      <List label="Pending invitations">
        <ListRow title="rui@example.com" meta="Member · expires in 6 days" />
        <ListRow
          muted
          title="marta@example.com"
          meta="Admin · expired"
          trailing={<Button variant="ghost">Send again</Button>}
        />
      </List>
    </div>
  ),
};

/** Mailboxes: the chosen row (J/K) is highlighted and carries the keys. */
export const Selected: Story = {
  render: () => (
    <div className="max-w-measure">
      <List label="Mailboxes">
        <ListRow
          leading={<ProviderMark name="Gmail" size="lg" />}
          title="ana@example.com"
          meta="Gmail · Connected by you"
          trailing={<Button variant="ghost">Disconnect</Button>}
        >
          <StatusText tone="ok">Syncing</StatusText>
        </ListRow>
        <ListRow
          selected
          leading={<ProviderMark name="Zoho Mail" size="lg" />}
          title="ana.duarte@example.net"
          meta="Zoho Mail · Connected by you"
          trailing={
            <>
              <Button variant="secondary" keys="r">
                Reconnect
              </Button>
              <Button variant="ghost" keys="backspace">
                Disconnect
              </Button>
            </>
          }
        >
          <StatusText tone="problem">Needs you to sign in again</StatusText>
        </ListRow>
      </List>
    </div>
  ),
};
