import { mergeAttributes, Node as BlockNode, type Attributes } from "@tiptap/core";
import {
  NodeViewContent,
  NodeViewWrapper,
  ReactNodeViewRenderer,
  type NodeViewProps,
} from "@tiptap/react";
import { useTranslation } from "react-i18next";

import { browserUrl } from "~/lib/api";
import {
  DocCard,
  DocImage,
  DocToggle,
  MentionChip,
  type MentionChipProps,
  type MentionKind,
} from "~/lib/ui";

import { useRecordLabel, type RecordLabel } from "../api/records";
import { isMentionType, type MentionType } from "../types";

import { useEditorEnvironment } from "./environment";

/** A node attribute that is a plain string, kept in a `data-` attribute in HTML. */
function stringAttribute(name: string, fallback: string | null): Attributes[string] {
  return {
    default: fallback,
    parseHTML: (element) => element.getAttribute(`data-${name.replaceAll("_", "-")}`) ?? fallback,
    renderHTML: (attributes) => {
      const value: unknown = attributes[name];
      return typeof value === "string" ? { [`data-${name.replaceAll("_", "-")}`]: value } : {};
    },
  };
}

const text = (value: unknown): string => (typeof value === "string" ? value : "");

/** The look of a mention for each kind of record. */
const MENTION_KINDS = {
  "workspaces.member": "person",
  person: "person",
  "conversations.conversation": "thread",
  "tasks.task": "task",
  "schedule.event": "event",
  "pages.page": "page",
} as const satisfies Record<MentionType, MentionKind>;

function useLabelOf(attributes: Record<string, unknown>): {
  type: MentionType | undefined;
  id: string;
  label: RecordLabel;
} {
  const environment = useEditorEnvironment();
  const type = text(attributes["ref_type"]);
  const id = text(attributes["ref_id"]);
  const known = isMentionType(type) ? type : undefined;
  const label = useRecordLabel(environment.workspaceId, known ?? "person", id);
  return { type: known, id, label: known === undefined ? { state: "closed", label: "" } : label };
}

/** The chip's props: its name, where it opens, or "Something you can't open". */
function chipOf(
  kind: MentionKind,
  label: RecordLabel,
  closedText: string,
  open: (href: string) => void,
): MentionChipProps {
  return {
    kind,
    state: label.state,
    label: label.state === "closed" ? closedText : label.label,
    href: label.href,
    onOpen: (event) => {
      if (label.href !== undefined && !event.metaKey && !event.ctrlKey && event.button === 0) {
        event.preventDefault();
        open(label.href);
      }
    },
  };
}

function MentionView({ node }: NodeViewProps) {
  const { t } = useTranslation("pages");
  const environment = useEditorEnvironment();
  const { type, label } = useLabelOf(node.attrs);
  return (
    <NodeViewWrapper as="span">
      <MentionChip
        {...chipOf(
          type === undefined ? "person" : MENTION_KINDS[type],
          label,
          t("mention.closed"),
          environment.open,
        )}
      />
    </NodeViewWrapper>
  );
}

/** A person, thread, task, event or page named inline: a reference, never a copy. */
export const Mention = BlockNode.create({
  name: "mention",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,
  addAttributes() {
    return { ref_type: stringAttribute("ref_type", null), ref_id: stringAttribute("ref_id", null) };
  },
  parseHTML() {
    return [{ tag: 'span[data-type="mention"]' }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes(HTMLAttributes, { "data-type": "mention" })];
  },
  addNodeView() {
    return ReactNodeViewRenderer(MentionView);
  },
});

const EMBED_ICONS = {
  "workspaces.member": "people",
  person: "people",
  "conversations.conversation": "mail",
  "tasks.task": "tasks",
  "schedule.event": "event",
  "pages.page": "document",
} as const satisfies Record<MentionType, "people" | "mail" | "tasks" | "event" | "document">;

function EmbedView({ node }: NodeViewProps) {
  const { t } = useTranslation("pages");
  const environment = useEditorEnvironment();
  const { type, label } = useLabelOf(node.attrs);
  const chip = chipOf("page", label, t("mention.closed"), environment.open);
  return (
    <NodeViewWrapper>
      <DocCard
        icon={type === undefined ? "document" : EMBED_ICONS[type]}
        title={chip.label}
        meta={type === undefined ? undefined : t(`embed.${MENTION_KINDS[type]}`)}
        href={chip.href}
        onOpen={chip.onOpen}
        state={chip.state}
      />
    </NodeViewWrapper>
  );
}

/** A block that shows a record (a thread, a task, an event) as a card. */
export const Embed = BlockNode.create({
  name: "embed",
  group: "block",
  atom: true,
  selectable: true,
  draggable: true,
  addAttributes() {
    return { ref_type: stringAttribute("ref_type", null), ref_id: stringAttribute("ref_id", null) };
  },
  parseHTML() {
    return [{ tag: 'div[data-type="embed"]' }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, { "data-type": "embed" })];
  },
  addNodeView() {
    return ReactNodeViewRenderer(EmbedView);
  },
});

function PageLinkView({ node }: NodeViewProps) {
  const { t } = useTranslation("pages");
  const environment = useEditorEnvironment();
  const label = useRecordLabel(environment.workspaceId, "pages.page", text(node.attrs["page"]));
  const chip = chipOf("page", label, t("mention.closed"), environment.open);
  return (
    <NodeViewWrapper>
      <DocCard
        icon="document"
        title={chip.label === "" ? t("untitled") : chip.label}
        meta={t("embed.page")}
        href={chip.href}
        onOpen={chip.onOpen}
        state={chip.state}
      />
    </NodeViewWrapper>
  );
}

/** A link to a sub-page, as a card. */
export const PageLink = BlockNode.create({
  name: "pageLink",
  group: "block",
  atom: true,
  selectable: true,
  draggable: true,
  addAttributes() {
    return { page: stringAttribute("page", null) };
  },
  parseHTML() {
    return [{ tag: 'div[data-type="page-link"]' }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, { "data-type": "page-link" })];
  },
  addNodeView() {
    return ReactNodeViewRenderer(PageLinkView);
  },
});

const numberAttribute = (name: string): Attributes[string] => ({
  default: 0,
  parseHTML: (element) => Number(element.getAttribute(`data-${name}`) ?? 0),
  renderHTML: (attributes) => {
    const value: unknown = attributes[name];
    return typeof value === "number" ? { [`data-${name}`]: String(value) } : {};
  },
});

/** The address a page's file is served from, through the BFF with the viewer's cookie. */
function fileAddress(workspaceId: string, pageId: string, blob: string): string | undefined {
  return browserUrl(`/api/v1/w/${workspaceId}/pages/${pageId}/files/${blob}`);
}

const kilobytes = (bytes: number, language: string) =>
  new Intl.NumberFormat(language, { style: "unit", unit: "kilobyte", maximumFractionDigits: 0 }).format(
    Math.max(1, Math.round(bytes / 1024)),
  );

function FileView({ node }: NodeViewProps) {
  const { i18n } = useTranslation("pages");
  const environment = useEditorEnvironment();
  const name = text(node.attrs["name"]);
  const size = Number(node.attrs["size"]);
  return (
    <NodeViewWrapper>
      <DocCard
        icon="attachment"
        title={name}
        meta={Number.isFinite(size) ? kilobytes(size, i18n.language) : undefined}
        href={fileAddress(environment.workspaceId, environment.pageId, text(node.attrs["blob"]))}
        download={name}
      />
    </NodeViewWrapper>
  );
}

const FILE_ATTRIBUTES = () => ({
  blob: stringAttribute("blob", null),
  name: stringAttribute("name", ""),
  size: numberAttribute("size"),
});

/** A file the page holds, as a card that downloads it. */
export const FileBlock = BlockNode.create({
  name: "file",
  group: "block",
  atom: true,
  selectable: true,
  draggable: true,
  addAttributes: FILE_ATTRIBUTES,
  parseHTML() {
    return [{ tag: 'div[data-type="file"]' }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, { "data-type": "file" })];
  },
  addNodeView() {
    return ReactNodeViewRenderer(FileView);
  },
});

function ImageView({ node }: NodeViewProps) {
  const environment = useEditorEnvironment();
  const name = text(node.attrs["name"]);
  const src = fileAddress(environment.workspaceId, environment.pageId, text(node.attrs["blob"]));
  return (
    <NodeViewWrapper>
      {src === undefined ? (
        <DocCard icon="image" title={name} />
      ) : (
        <DocImage src={src} alt={name} />
      )}
    </NodeViewWrapper>
  );
}

/** An image the page holds. */
export const ImageBlock = BlockNode.create({
  name: "image",
  group: "block",
  atom: true,
  selectable: true,
  draggable: true,
  addAttributes: FILE_ATTRIBUTES,
  parseHTML() {
    return [{ tag: 'div[data-type="image"]' }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, { "data-type": "image" })];
  },
  addNodeView() {
    return ReactNodeViewRenderer(ImageView);
  },
});

/** A coloured note in the text. */
export const Callout = BlockNode.create({
  name: "callout",
  group: "block",
  content: "block+",
  defining: true,
  addAttributes() {
    return { tone: stringAttribute("tone", "neutral") };
  },
  parseHTML() {
    return [{ tag: 'div[data-type="callout"]' }];
  },
  renderHTML({ HTMLAttributes }) {
    return [
      "div",
      mergeAttributes({ "data-type": "callout" }, HTMLAttributes, {
        "data-tone": text(HTMLAttributes["data-tone"]) || "neutral",
      }),
      0,
    ];
  },
});

function ToggleView({ node, updateAttributes }: NodeViewProps) {
  const { t } = useTranslation("pages");
  const open = node.attrs["open"] === true;
  return (
    <NodeViewWrapper>
      <DocToggle
        open={open}
        label={t(open ? "toggle.hide" : "toggle.show")}
        onToggle={() => {
          updateAttributes({ open: !open });
        }}
      >
        <NodeViewContent />
      </DocToggle>
    </NodeViewWrapper>
  );
}

/** A block whose first line hides what follows it. */
export const Toggle = BlockNode.create({
  name: "toggle",
  group: "block",
  content: "block+",
  defining: true,
  addAttributes() {
    return {
      open: {
        default: false,
        parseHTML: (element) => element.getAttribute("data-open") === "true",
        renderHTML: (attributes) => ({ "data-open": String(attributes["open"] === true) }),
      },
    };
  },
  parseHTML() {
    return [{ tag: 'div[data-type="toggle"]' }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, { "data-type": "toggle" }), 0];
  },
  addNodeView() {
    return ReactNodeViewRenderer(ToggleView);
  },
});
