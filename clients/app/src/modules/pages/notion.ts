import type { NotionStopReason } from "./types";

/** The address of a Notion page: its id without dashes. */
export function notionUrl(notionPage: string): string {
  return `https://www.notion.so/${notionPage.replaceAll("-", "")}`;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** "now", "1 min ago", "3 hours ago", "2 days ago": how long before `now` a time was. */
export function agoLabel(when: string | Date, now: Date, locale: string): string {
  const ms = Math.max(0, now.getTime() - new Date(when).getTime());
  const format = new Intl.RelativeTimeFormat(locale, { numeric: "auto", style: "short" });
  if (ms < MINUTE) {
    return format.format(0, "second");
  }
  if (ms < HOUR) {
    return format.format(-Math.floor(ms / MINUTE), "minute");
  }
  if (ms < DAY) {
    return format.format(-Math.floor(ms / HOUR), "hour");
  }
  return format.format(-Math.floor(ms / DAY), "day");
}

/** When the link last moved either way (the newer of the two), if it ever did. */
export function lastSyncAt(link: {
  last_inbound_at?: string | null | undefined;
  last_outbound_at?: string | null | undefined;
}): string | undefined {
  const times = [link.last_inbound_at, link.last_outbound_at].filter(
    (time): time is string => typeof time === "string",
  );
  return times.toSorted((a, b) => new Date(b).getTime() - new Date(a).getTime())[0];
}

/** The reasons a link ends that this client words; anything newer reads as the general one. */
const REASONS = [
  "stopped_by_member",
  "disconnected",
  "trashed_here",
  "trashed_in_notion",
  "not_shared",
  "member_left",
  "notion_unavailable",
] as const satisfies readonly NotionStopReason[];

export type KnownReason = (typeof REASONS)[number];

export function knownReason(reason?: string | null): KnownReason | undefined {
  return REASONS.find((known) => known === reason);
}

/** A page of the listing, as the import shows it. */
export interface ImportPage {
  notion_page: string;
  title: string;
  parent?: string | null;
  sub_pages: number;
  linked: boolean;
}

/** A listed page with the listed pages under it. */
export interface ImportNode {
  page: ImportPage;
  depth: number;
  children: ImportNode[];
}

/** The listing as a tree, in the order Notion listed it (newest edit first); a parent not listed makes a root. */
export function importTree(pages: readonly ImportPage[]): ImportNode[] {
  const known = new Set(pages.map((page) => page.notion_page));
  const children = new Map<string, ImportPage[]>();
  const roots: ImportPage[] = [];
  for (const page of pages) {
    const parent = page.parent;
    if (typeof parent === "string" && known.has(parent) && parent !== page.notion_page) {
      children.set(parent, [...(children.get(parent) ?? []), page]);
    } else {
      roots.push(page);
    }
  }
  const seen = new Set<string>();
  const build = (page: ImportPage, depth: number): ImportNode => {
    seen.add(page.notion_page);
    return {
      page,
      depth,
      children: (children.get(page.notion_page) ?? [])
        .filter((child) => !seen.has(child.notion_page))
        .map((child) => build(child, depth + 1)),
    };
  };
  const built = roots.map((root) => build(root, 0));
  // Pages under each other in a loop are no tree (Notion's are): none is lost.
  for (const page of pages) {
    if (!seen.has(page.notion_page)) {
      built.push(build(page, 0));
    }
  }
  return built;
}

/** The node and every node under it, in order. */
export function flatten(node: ImportNode): ImportNode[] {
  return [node, ...node.children.flatMap(flatten)];
}

/** Every node of the tree, in order. */
export function allNodes(tree: readonly ImportNode[]): ImportNode[] {
  return tree.flatMap(flatten);
}

/** The pages ticked, and what ticking one means for the ones under it. */
export interface Choice {
  /** The ones ticked by hand. */
  ticked: ReadonlySet<string>;
  /** Ticked, or under a ticked page: they all come along. */
  included: ReadonlySet<string>;
}

/**
 * Tick or untick `node`. Ticking a page takes its sub-pages along, so they
 * are not ticked by hand any more; unticking a page that came along because
 * an ancestor was ticked is not possible (untick the ancestor).
 */
export function toggle(
  tree: readonly ImportNode[],
  ticked: ReadonlySet<string>,
  id: string,
): Set<string> {
  const node = allNodes(tree).find((candidate) => candidate.page.notion_page === id);
  const next = new Set(ticked);
  if (node === undefined) {
    return next;
  }
  if (next.has(id)) {
    next.delete(id);
    return next;
  }
  if (node.page.linked) {
    return next;
  }
  for (const below of flatten(node)) {
    next.delete(below.page.notion_page);
  }
  next.add(id);
  return next;
}

/** What ticking comes to: which pages are included, and which are asked for. */
export function choiceOf(tree: readonly ImportNode[], ticked: ReadonlySet<string>): Choice {
  const included = new Set<string>();
  const walk = (node: ImportNode, inherited: boolean) => {
    const on = inherited || ticked.has(node.page.notion_page);
    if (on && !node.page.linked) {
      included.add(node.page.notion_page);
    }
    for (const child of node.children) {
      walk(child, on);
    }
  };
  for (const root of tree) {
    walk(root, false);
  }
  return { ticked, included };
}

/** The pages to ask for: the ticked ones that are not under another ticked one, and are not linked. */
export function importRequest(
  tree: readonly ImportNode[],
  ticked: ReadonlySet<string>,
): { notion_page: string; title: string }[] {
  const asked: { notion_page: string; title: string }[] = [];
  const walk = (node: ImportNode, inherited: boolean) => {
    const own = ticked.has(node.page.notion_page);
    if (own && !inherited && !node.page.linked) {
      asked.push({ notion_page: node.page.notion_page, title: node.page.title.slice(0, 200) });
    }
    for (const child of node.children) {
      walk(child, inherited || own);
    }
  };
  for (const root of tree) {
    walk(root, false);
  }
  return asked;
}
