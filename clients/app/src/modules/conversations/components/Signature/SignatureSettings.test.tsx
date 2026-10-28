import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { editorOf } from "../../../../../test/composer";
import { INBOX, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { SignatureSettings } from "./SignatureSettings";

const SIGNATURE = `${W}/inboxes/${INBOX.id}/signature`;
const READER = { ...INBOX, id: "0192f1a0-0000-7000-8000-0000000000b2", role: "reader" };

describe("SignatureSettings", () => {
  it("gives each mailbox you can send from its own editor, opened with what you kept", async () => {
    mockApi({
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX, READER] } },
      [`GET ${SIGNATURE}`]: { body: { text: "**Sam Rivera**" } },
    });
    renderWithProviders(<SignatureSettings workspaceId={DUARTE.id} />);
    const box = await screen.findByRole("textbox", {
      name: "Your signature for sam.rivera@example.com",
    });
    expect(box.querySelector("strong")).toHaveTextContent("Sam Rivera");
    // A mailbox you only read has none.
    expect(screen.getAllByRole("textbox")).toHaveLength(1);
  });

  it("saves with Save and with Ctrl+Enter, and says so", async () => {
    const api = mockApi({
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`GET ${SIGNATURE}`]: { body: { text: "" } },
      [`PUT ${SIGNATURE}`]: (_request, body) => ({ body }),
    });
    renderWithProviders(<SignatureSettings workspaceId={DUARTE.id} />);
    const box = await screen.findByRole("textbox", {
      name: "Your signature for sam.rivera@example.com",
    });
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();
    act(() => {
      editorOf(box).chain().focus().insertContent("Sam Rivera").run();
    });
    expect(save).toBeEnabled();
    fireEvent.click(save);
    expect(await screen.findByText("Signature saved")).toBeInTheDocument();
    expect(api.calls.find((call) => call.route.startsWith("PUT"))?.body).toEqual({
      text: "Sam Rivera",
    });
    // Changed again, saved from the keyboard.
    act(() => {
      editorOf(box).chain().focus().insertContent(" · Rivers Lettings").run();
    });
    fireEvent.keyDown(box, { key: "Enter", keyCode: 13, ctrlKey: true });
    await waitFor(() => {
      expect(api.calls.filter((call) => call.route.startsWith("PUT"))).toHaveLength(2);
    });
    expect(api.calls.findLast((call) => call.route.startsWith("PUT"))?.body).toEqual({
      text: "Sam Rivera · Rivers Lettings",
    });
  });

  it("states the limit, refuses a longer text and says when the server did", async () => {
    mockApi({
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`GET ${SIGNATURE}`]: { body: { text: "" } },
      [`PUT ${SIGNATURE}`]: problem(400, "signature_too_long"),
    });
    renderWithProviders(<SignatureSettings workspaceId={DUARTE.id} />);
    const box = await screen.findByRole("textbox", {
      name: "Your signature for sam.rivera@example.com",
    });
    expect(screen.getByText("0 of 2000 characters")).toBeInTheDocument();
    act(() => {
      editorOf(box).chain().focus().insertContent("a".repeat(2001)).run();
    });
    expect(screen.getByRole("alert")).toHaveTextContent(
      "A signature can have 2000 characters at most.",
    );
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("says when the server refused it", async () => {
    mockApi({
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`GET ${SIGNATURE}`]: { body: { text: "" } },
      [`PUT ${SIGNATURE}`]: problem(400, "signature_too_long"),
    });
    renderWithProviders(<SignatureSettings workspaceId={DUARTE.id} />);
    const box = await screen.findByRole("textbox", {
      name: "Your signature for sam.rivera@example.com",
    });
    act(() => {
      editorOf(box).chain().focus().insertContent("Sam").run();
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("A signature can have 2000");
  });
});
