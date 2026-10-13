import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DUARTE, HOUSEHOLD } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { CreateWorkspace, type CreateWorkspaceProps } from "./CreateWorkspace";

const CREATE = "POST /bff/api/v1/workspaces";

function renderFlow(props: Partial<CreateWorkspaceProps> = {}) {
  const handlers = {
    onOpen: vi.fn(),
    onInvite: vi.fn(),
    onSignOut: vi.fn(),
    ...props,
  };
  renderWithProviders(<CreateWorkspace firstRun {...handlers} />);
  return handlers;
}

describe("CreateWorkspace", () => {
  it("asks the name, then the kind, then opens the new workspace", async () => {
    const api = mockApi({ [CREATE]: { status: 201, body: HOUSEHOLD } });
    const { onOpen, onInvite } = renderFlow();

    const name = await screen.findByRole("textbox", { name: "Workspace name" });
    await waitFor(() => {
      expect(name).toHaveFocus();
    });
    expect(screen.getByText("Step 1 of 2")).toBeVisible();
    fireEvent.change(name, { target: { value: "  Household " } });
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));

    expect(await screen.findByRole("heading", { level: 1, name: "What is it for?" })).toBeVisible();
    fireEvent.click(screen.getByRole("radio", { name: "Personal" }));
    expect(screen.getByRole("main").parentElement).toHaveAttribute("data-accent", "personal");
    fireEvent.click(screen.getByRole("button", { name: "Create workspace" }));

    expect(
      await screen.findByRole("heading", { level: 1, name: "Household is ready" }),
    ).toBeVisible();
    expect(api.calls).toEqual([{ route: CREATE, body: { name: "Household", kind: "personal" } }]);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Open Household" })).toHaveFocus();
    });
    fireEvent.click(screen.getByRole("button", { name: "Open Household" }));
    fireEvent.click(screen.getByRole("button", { name: "Invite people" }));
    expect(onOpen).toHaveBeenCalledWith(HOUSEHOLD);
    expect(onInvite).toHaveBeenCalledWith(HOUSEHOLD);
  });

  it("creates with defaults when both questions are skipped", async () => {
    const api = mockApi({ [CREATE]: { status: 201, body: DUARTE } });
    renderFlow();

    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));

    await screen.findByRole("heading", { level: 1, name: "Duarte & Pessoa is ready" });
    expect(api.calls[0]?.body).toEqual({ name: "My workspace", kind: "business" });
  });

  it("refuses a name over 100 characters and goes back from the kind", async () => {
    renderFlow();
    const name = await screen.findByRole("textbox", { name: "Workspace name" });

    fireEvent.change(name, { target: { value: "x".repeat(101) } });
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(name).toHaveAccessibleDescription(/Keep it to 100 characters or fewer/u);

    fireEvent.change(name, { target: { value: "Quinta" } });
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    fireEvent.click(await screen.findByRole("button", { name: "Back" }));
    expect(await screen.findByRole("textbox", { name: "Workspace name" })).toHaveValue("Quinta");
  });

  it("asks for a signup code when the instance needs one", async () => {
    const api = mockApi({
      [CREATE]: (_request, body) =>
        (body as { signup_code?: string }).signup_code === "friends"
          ? { status: 201, body: DUARTE }
          : problem(403, "signup_code_invalid"),
    });
    renderFlow();
    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create workspace" }));

    const code = await screen.findByRole("textbox", { name: "Signup code" });
    expect(code).not.toHaveAttribute("aria-invalid");
    fireEvent.change(code, { target: { value: "nope" } });
    fireEvent.click(screen.getByRole("button", { name: "Create workspace" }));
    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "Signup code" })).toHaveAccessibleDescription(
        "That code didn’t work. Check it and try again.",
      );
    });

    fireEvent.change(screen.getByRole("textbox", { name: "Signup code" }), {
      target: { value: " friends " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create workspace" }));
    await screen.findByRole("heading", { level: 1, name: "Duarte & Pessoa is ready" });
    expect(api.calls.at(-1)?.body).toEqual({
      name: "My workspace",
      kind: "business",
      signup_code: "friends",
    });
  });

  it("explains an invite-only instance and offers to sign out", async () => {
    mockApi({ [CREATE]: problem(403, "signup_invite_only") });
    const { onSignOut } = renderFlow();
    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));

    expect(
      await screen.findByRole("heading", { level: 1, name: "You need an invitation" }),
    ).toBeVisible();
    fireEvent.click(within(screen.getByRole("main")).getByRole("button", { name: "Sign out" }));
    expect(onSignOut).toHaveBeenCalled();
  });

  it("explains a pending approval and checks again", async () => {
    const api = mockApi({ [CREATE]: problem(403, "signup_requires_approval") });
    renderFlow();
    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));

    await screen.findByRole("heading", { level: 1, name: "Your request is waiting" });
    fireEvent.click(screen.getByRole("button", { name: "Check again" }));
    await waitFor(() => {
      expect(api.calls).toHaveLength(2);
    });
  });

  it("says when Campbooks didn't answer, and can be cancelled when not the first", async () => {
    mockApi({ [CREATE]: { status: 500, body: { title: "Internal", status: 500 } } });
    const onCancel = vi.fn();
    renderWithProviders(
      <CreateWorkspace
        firstRun={false}
        onCancel={onCancel}
        onOpen={vi.fn()}
        onInvite={vi.fn()}
        onSignOut={vi.fn()}
      />,
    );

    expect(
      await screen.findByRole("heading", { name: "What should we call the new workspace?" }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create workspace" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn’t create the workspace");

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalled();
  });

  it("hands the new workspace to a step added after creating it", async () => {
    mockApi({ [CREATE]: { status: 201, body: HOUSEHOLD } });
    const onOpen = vi.fn();
    renderWithProviders(
      <CreateWorkspace
        firstRun
        onOpen={onOpen}
        onInvite={vi.fn()}
        onSignOut={vi.fn()}
        afterCreate={({ workspace, onContinue, children }) => (
          <>
            <button type="button" onClick={onContinue}>
              {`Next for ${workspace.name}`}
            </button>
            {children}
          </>
        )}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("button", { name: "Skip" }));
    fireEvent.click(await screen.findByRole("button", { name: "Next for Household" }));

    expect(onOpen).toHaveBeenCalledWith(HOUSEHOLD);
    expect(screen.getByRole("heading", { level: 1, name: "Household is ready" })).toBeVisible();
  });
});
