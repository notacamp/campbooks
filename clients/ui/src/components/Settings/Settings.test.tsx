import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../test/render";

import { PrivacyLine, SettingsEmpty, SettingsPage, SettingsSection } from "./Settings";

describe("SettingsPage and SettingsSection", () => {
  it("names the page and each card by its title, with the count and description", () => {
    renderUi(
      <SettingsPage label="Ways to sign in" title="Ways to sign in" lead="How you get in.">
        <SettingsSection title="Accounts" count={2} description="Any of these works.">
          <p>body</p>
        </SettingsSection>
        <SettingsSection title="Passkeys" headingLevel={3}>
          <SettingsEmpty title="No passkeys yet.">Add one.</SettingsEmpty>
        </SettingsSection>
      </SettingsPage>,
    );

    expect(screen.getByRole("main", { name: "Ways to sign in" })).toBeVisible();
    expect(screen.getByRole("heading", { level: 1, name: "Ways to sign in" })).toBeVisible();
    const accounts = screen.getByRole("region", { name: /^Accounts/u });
    expect(within(accounts).getByRole("heading", { level: 2 })).toHaveTextContent("Accounts · 2");
    expect(within(accounts).getByText("Any of these works.")).toBeVisible();
    const passkeys = screen.getByRole("region", { name: "Passkeys" });
    expect(within(passkeys).getByRole("heading", { level: 3 })).toBeVisible();
    expect(within(passkeys).getByText("No passkeys yet.")).toBeVisible();
  });

  it("puts page actions beside the title and lets a section take a title id", () => {
    renderUi(
      <SettingsPage label="Members" title="Members" actions={<button type="button">Invite</button>}>
        <SettingsSection title="Invite" titleId="invite-title">
          <PrivacyLine>Only the address is sent.</PrivacyLine>
        </SettingsSection>
      </SettingsPage>,
    );

    expect(screen.getByRole("button", { name: "Invite" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "Invite" })).toHaveAttribute("id", "invite-title");
    expect(screen.getByText("Only the address is sent.")).toBeVisible();
  });
});
