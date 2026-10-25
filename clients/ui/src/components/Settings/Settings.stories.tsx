import type { Meta, StoryObj } from "@storybook/react-vite";

import { Button } from "../Button/Button";
import { List, ListRow } from "../List/List";
import { TextField } from "../Field/Field";

import { PrivacyLine, SettingsEmpty, SettingsPage, SettingsSection } from "./Settings";

const meta = {
  title: "Components/Settings",
  component: SettingsPage,
  parameters: { layout: "fullscreen" },
  args: { label: "Scout’s memory", title: "Scout’s memory", children: null },
} satisfies Meta<typeof SettingsPage>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Page: Story = {
  args: {
    lead: "What Scout knows about how you want your mail handled. Rules run first on every email, before any AI, and always give the same result.",
    children: (
      <>
        <SettingsSection
          title="Teach Scout something"
          description="Say it in a sentence. Scout shows you the rule, and what it matches, before anything is saved."
        >
          <TextField
            label="What should Scout do?"
            hideLabel
            placeholder="E.g. anything from my accountant matters"
          />
          <PrivacyLine>Only your sentence goes to the AI, never your mail.</PrivacyLine>
        </SettingsSection>
        <SettingsSection title="Your rules" description="Everything Scout does on its own.">
          <SettingsEmpty title="No rules yet">
            Teach Scout above, or answer its questions below.
          </SettingsEmpty>
        </SettingsSection>
      </>
    ),
  },
};

export const WithCountAndActions: Story = {
  args: {
    title: "Ways to sign in",
    lead: "How you get into Campbooks. Keep at least two, so losing one never locks you out.",
    actions: <Button icon="plus">Invite a member</Button>,
    children: (
      <>
        <SettingsSection
          title="Accounts"
          count={2}
          description="You can sign in with any of these."
        >
          <List label="Accounts">
            <ListRow title="Google" meta="sam.rivera@example.com · last used Oct 16" />
            <ListRow title="Zoho" meta="sam@rivera-studio.example · last used Oct 14" />
          </List>
        </SettingsSection>
        <SettingsSection title="Passkeys" description="Sign in with your face or fingerprint.">
          <SettingsEmpty title="No passkeys yet." action={<Button>Add a passkey</Button>}>
            Add one on this device.
          </SettingsEmpty>
        </SettingsSection>
        <SettingsSection title="Sign out everywhere" description="Ends every sign-in.">
          <div>
            <Button variant="danger-outline" icon="sign-out">
              Sign out everywhere
            </Button>
          </div>
        </SettingsSection>
      </>
    ),
  },
};
