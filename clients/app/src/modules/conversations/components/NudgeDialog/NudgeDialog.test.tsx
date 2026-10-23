import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { Schema } from "~/lib/api";
import { ScoutChatProvider } from "~/lib/scout-chat";

import { ANA, ANA_DETAIL } from "../../../../../test/demo/attention";
import { INBOX, W } from "../../../../../test/demo/conversations";
import { REPLY } from "../../../../../test/demo/drafts";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { NudgeDialog, type NudgeResume } from "./NudgeDialog";

const C = `${W}/conversations/${ANA.id}`;
const D = `${W}/drafts`;
const WAITED = ANA.need?.message_id ?? "";
const TEXT = "Hi Ana,\n\nCould you send me the revised floor plan before Thursday?\n\nThanks,\nSam";
const SHORT = "Hi Ana, any news on the floor plan?\n\nSam";
const ANA_ADDRESS = { name: "Ana Costa", address: "ana@costa-arquitetos.example" };
const JOAO_ADDRESS = { name: "João Matos", address: "joao@obras-matos.example" };

/** The nudge draft the dialog creates. */
const NUDGE: Schema<"DraftView"> = {
  ...REPLY,
  id: "0192f1a0-0000-7000-8000-000000000201",
  kind: "nudge",
  conversation_id: ANA.id,
  message_id: WAITED,
  to: [ANA_ADDRESS],
  cc: [JOAO_ADDRESS],
  subject: `Re: ${ANA.subject}`,
  content_token: "demo-token-nudge",
};

function routes(extra: Parameters<typeof mockApi>[0] = {}) {
  return mockApi({
    [`GET ${C}`]: { body: ANA_DETAIL },
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${D}`]: { body: { drafts: [] } },
    [`POST ${C}/nudge/draft`]: (_request, body) => ({
      body: {
        message_id: WAITED,
        text: (body as { shorter?: boolean }).shorter === true ? SHORT : TEXT,
      },
    }),
    [`POST ${D}`]: (_request, body) => ({
      status: 201,
      body: { ...NUDGE, body: (body as { body: string }).body },
    }),
    [`POST ${D}/${NUDGE.id}/send`]: {
      status: 202,
      body: { ...NUDGE, body: SHORT, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
    },
    [`POST ${D}/${NUDGE.id}/cancel_send`]: { body: { ...NUDGE, body: SHORT, version: 3 } },
    ...extra,
  });
}

function dialog(props: Partial<Parameters<typeof NudgeDialog>[0]> = {}) {
  const handlers = {
    onClose: vi.fn(),
    onOpenDraft: vi.fn(),
    onSent: vi.fn(),
    onUndone: vi.fn(),
  };
  if (ANA.need === null || ANA.need === undefined) {
    throw new Error("the fixture has no need");
  }
  renderWithProviders(
    <NudgeDialog
      workspaceId={DUARTE.id}
      conversationId={ANA.id}
      need={ANA.need}
      {...handlers}
      {...props}
    />,
  );
  return handlers;
}

describe("NudgeDialog", () => {
  it("asks Scout about the conversation from its header, and stays open", async () => {
    routes();
    const scout = vi.fn();
    const onClose = vi.fn();
    if (ANA.need === null || ANA.need === undefined) {
      throw new Error("the fixture has no need");
    }
    renderWithProviders(
      <ScoutChatProvider open={scout}>
        <NudgeDialog
          workspaceId={DUARTE.id}
          conversationId={ANA.id}
          need={ANA.need}
          onClose={onClose}
          onOpenDraft={vi.fn()}
        />
      </ScoutChatProvider>,
    );
    const nudge = await screen.findByRole("dialog", { name: "Nudge Ana" });
    const ask = within(nudge).getByRole("button", { name: "Ask Scout about this email" });

    fireEvent.click(ask);
    expect(scout).toHaveBeenCalledWith(
      { type: "conversations.conversation", id: ANA.id },
      { from: ask },
    );
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Nudge Ana" })).toBeVisible();
  });

  it("writes Scout's nudge, says who it goes to and who is copied, and sends it with undo", async () => {
    const api = routes();
    const handlers = dialog();
    // Scout writes as soon as it opens.
    expect(screen.getByText("Scout is writing a short nudge…")).toBeInTheDocument();

    expect(await screen.findByRole("dialog", { name: "Nudge Ana" })).toBeVisible();
    const text = await screen.findByRole("textbox", { name: "Your nudge" });
    await waitFor(() => {
      expect(text).toHaveValue(TEXT);
    });
    expect(api.calls.find((c) => c.route === `POST ${C}/nudge/draft`)?.body).toEqual({});
    // To the awaited person, copying the others; never the inbox itself.
    const ana = await screen.findByText("Ana Costa <ana@costa-arquitetos.example>");
    expect(ana.tagName).toBe("BDI");
    expect(ana.closest("p")).toHaveTextContent("To Ana Costa <ana@costa-arquitetos.example>");
    expect(
      screen.getByText("João Matos <joao@obras-matos.example>").closest("p"),
    ).toHaveTextContent("Copying João Matos <joao@obras-matos.example>");
    // The line it follows up on.
    expect(screen.getByText(/Could you send Sam the revised floor plan directly/u)).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: /Shorter/u }));
    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "Your nudge" })).toHaveValue(SHORT);
    });
    expect(api.calls.findLast((c) => c.route === `POST ${C}/nudge/draft`)?.body).toEqual({
      draft: TEXT,
      shorter: true,
      change: null,
    });
    // Nothing is drafted before Send.
    expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    const created = api.calls.find((c) => c.route === `POST ${D}`)?.body as {
      quoted: string;
    };
    expect(created).toMatchObject({ kind: "nudge", message_id: WAITED, body: SHORT, bcc: [] });
    // The server addresses a nudge: the client sends no To or Cc.
    expect(created).not.toHaveProperty("to");
    expect(created).not.toHaveProperty("cc");
    expect(created.quoted).toMatch(/João Matos wrote:/u);
    expect(api.calls.map((c) => c.route)).toContain(`POST ${D}/${NUDGE.id}/send`);
    expect(screen.getAllByText("Sending to Ana").length).toBeGreaterThan(0);
    expect(await screen.findByText("Nudged Ana")).toBeVisible();

    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(handlers.onUndone).toHaveBeenCalledWith({
        draft: { ...NUDGE, body: SHORT, version: 3 },
      });
    });
  });

  it("closes with Esc and creates nothing", async () => {
    const api = routes();
    const handlers = dialog();
    const text = await screen.findByRole("textbox", { name: "Your nudge" });
    fireEvent.keyDown(text, { key: "Escape" });
    expect(handlers.onClose).toHaveBeenCalled();
    expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(false);
  });

  it("offers only Write my own when no model may read mail here, on the addressed nudge draft", async () => {
    const api = routes({ [`POST ${C}/nudge/draft`]: problem(409, "reply_unavailable") });
    const handlers = dialog();
    expect(
      await screen.findByText("Scout can’t write here, so you write this nudge."),
    ).toBeVisible();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByRole("button", { name: /^Send/u })).toBeNull();
    const own = await screen.findByRole("button", { name: /Write my own/u });
    await waitFor(() => {
      expect(own).toBeEnabled();
    });
    fireEvent.click(own);
    await waitFor(() => {
      expect(handlers.onOpenDraft).toHaveBeenCalled();
    });
    const created = api.calls.find((c) => c.route === `POST ${D}`)?.body;
    expect(created).toMatchObject({ kind: "nudge", message_id: WAITED, body: "" });
    expect(created).not.toHaveProperty("to");
  });

  it("opens the nudge the member already started, never writing over it", async () => {
    const started = { ...NUDGE, id: "0192f1a0-0000-7000-8000-000000000202", body: "My words" };
    const api = routes({ [`GET ${D}`]: { body: { drafts: [started] } } });
    const handlers = dialog();
    await screen.findByRole("textbox", { name: "Your nudge" });
    const own = screen.getByRole("button", { name: /Write my own/u });
    await waitFor(() => {
      expect(own).toBeEnabled();
    });
    fireEvent.click(own);
    await waitFor(() => {
      expect(handlers.onOpenDraft).toHaveBeenCalledWith(started);
    });
    expect(
      api.calls.some((c) => c.route === `POST ${D}` || c.route === `PATCH ${D}/${started.id}`),
    ).toBe(false);
  });

  it("says when the person already wrote back", async () => {
    routes({ [`POST ${C}/nudge/draft`]: problem(409, "not_waiting") });
    dialog();
    expect(
      await screen.findByText("Ana Costa already wrote back, so there’s nothing to nudge."),
    ).toBeVisible();
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("picks up an undone nudge on its own text, asking Scout nothing", async () => {
    const api = routes();
    const resume: NudgeResume = { draft: { ...NUDGE, body: SHORT } };
    dialog({ resume });
    expect(await screen.findByRole("textbox", { name: "Your nudge" })).toHaveValue(SHORT);
    expect(api.calls.some((c) => c.route === `POST ${C}/nudge/draft`)).toBe(false);
  });

  it("sends nothing when the server addresses it otherwise than shown, then sends what it shows", async () => {
    const elsewhere = {
      ...NUDGE,
      cc: [{ name: "Rita Lopes", address: "rita@obras-matos.example" }],
    };
    const api = routes({
      [`POST ${D}`]: { status: 201, body: elsewhere },
      [`PATCH ${D}/${NUDGE.id}`]: { body: { ...elsewhere, version: 2 } },
    });
    const handlers = dialog();
    const text = await screen.findByRole("textbox", { name: "Your nudge" });
    await waitFor(() => {
      expect(text).toHaveValue(TEXT);
    });
    const send = screen.getByRole("button", { name: /^Send/u });
    await waitFor(() => {
      expect(send).toBeEnabled();
    });
    fireEvent.click(send);
    expect(
      (
        await screen.findAllByText(
          "This nudge now goes to other people than shown. Check who it goes to and send again.",
        )
      ).length,
    ).toBeGreaterThan(0);
    expect(api.calls.map((c) => c.route)).not.toContain(`POST ${D}/${NUDGE.id}/send`);
    expect(handlers.onSent).not.toHaveBeenCalled();
    // It now shows where the draft goes; Send again sends that.
    expect(await screen.findByText("Rita Lopes <rita@obras-matos.example>")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    const saved = api.calls.find((c) => c.route === `PATCH ${D}/${NUDGE.id}`)?.body;
    expect(saved).not.toHaveProperty("to");
  });

  it("nudges nobody when it waits on the inbox's own address", async () => {
    const api = routes({ [`POST ${C}/nudge/draft`]: problem(409, "not_waiting") });
    if (ANA.need === null || ANA.need === undefined) {
      throw new Error("the fixture has no need");
    }
    dialog({
      need: { ...ANA.need, waiting_on: { name: "Sam Rivera", address: INBOX.address } },
    });
    expect(
      await screen.findByText("This waits on your own address, so there is nobody to nudge."),
    ).toBeVisible();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByRole("button", { name: /Write my own/u })).toBeNull();
    expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(false);
  });
});
