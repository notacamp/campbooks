import { StarterKit } from "@tiptap/starter-kit";
import { EditorContent, useEditor } from "@tiptap/react";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { NOTION_ROADMAP, notionLink } from "../../../../test/demo/pages";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../test/render";

import { Bid } from "./bid";
import { EditorEnvironmentContext } from "./environment";
import { Conflict, External } from "./nodes";

const PAGE = "page-1";

const content = {
  type: "doc",
  content: [
    {
      type: "paragraph",
      attrs: { bid: "b1" },
      content: [{ type: "text", text: "We launch on the 14th." }],
    },
    {
      type: "conflict",
      attrs: {
        of: "b1",
        hash: "ab".repeat(32),
        edited: "2026-10-25T09:00:00.000Z",
        removed: false,
      },
      content: [{ type: "paragraph", content: [{ type: "text", text: "We launch on the 21st." }] }],
    },
    { type: "external", attrs: { kind: "column_list" } },
  ],
};

function Editing({ editable = true }: { editable?: boolean }) {
  const editor = useEditor({
    extensions: [StarterKit, Bid, External, Conflict],
    editable,
    content,
  });
  return (
    <EditorEnvironmentContext value={{ workspaceId: DUARTE.id, pageId: PAGE, open: () => {} }}>
      <EditorContent editor={editor} />
    </EditorEnvironmentContext>
  );
}

describe("the Notion blocks in the editor", () => {
  it("shows a block kept in Notion with where to edit it", async () => {
    mockApi({
      [`GET ${W}/pages/notion/links`]: { body: { links: [notionLink(PAGE, NOTION_ROADMAP)] } },
      [`GET ${W}/pages/${PAGE}/notion/links`]: { body: { links: [] } },
    });
    renderWithProviders(<Editing />);
    expect(await screen.findByText("Kept in Notion · Columns")).toBeVisible();
    await waitFor(() => {
      expect(screen.getByRole("link", { name: "Edit in Notion" })).toHaveAttribute(
        "href",
        "https://www.notion.so/0192f1a00000700080000000000007a1",
      );
    });
  });

  it("shows a conflict with both versions and settles it", async () => {
    const mocked = mockApi({
      [`GET ${W}/pages/notion/links`]: { body: { links: [] } },
      [`POST ${W}/pages/${PAGE}/notion/conflicts/b1`]: {
        body: { resolution: "r1", revision: 3, conflicts: 0 },
      },
    });
    renderWithProviders(<Editing />);
    expect(await screen.findByText("We launch on the 21st.")).toBeVisible();
    expect(screen.getByText("We launch on the 14th.")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Use Notion’s" }));
    await waitFor(() => {
      expect(
        mocked.calls.find((call) => call.route === `POST ${W}/pages/${PAGE}/notion/conflicts/b1`)
          ?.body,
      ).toEqual({ keep: "notion" });
    });
    expect(await screen.findByText(/Using Notion’s version — it goes to Notion/u)).toBeVisible();
  });

  it("shows a reader the conflict without the choices", async () => {
    mockApi({ [`GET ${W}/pages/notion/links`]: { body: { links: [] } } });
    renderWithProviders(<Editing editable={false} />);
    expect(await screen.findByText("We launch on the 21st.")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Keep both" })).toBeNull();
  });
});
