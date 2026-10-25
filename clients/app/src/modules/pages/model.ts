import { generateKeyBetween } from "fractional-indexing";

import type { PageSummary } from "./types";

/** A page and the pages under it that the caller may open. */
export interface TreeNode {
  page: PageSummary;
  children: TreeNode[];
}

/** The tree's three groups. */
export interface Sections {
  workspace: TreeNode[];
  private: TreeNode[];
  shared: TreeNode[];
}

/** Positions are fractional keys: compare them by code unit, as the server does, never by locale. */
const compareKeys = (a: string, b: string): number => {
  if (a === b) {
    return 0;
  }
  return a < b ? -1 : 1;
};

/** Sibling order: by position, then by id. */
export const byPosition = (a: PageSummary, b: PageSummary): number =>
  compareKeys(a.position, b.position) || compareKeys(a.id, b.id);

/**
 * The flat list as the sidebar shows it. A top-level page is in Workspace,
 * or in Private when it is the caller's own. A page the caller was shared
 * into (another member's private page, or a page under one they cannot
 * open) is a root of "Shared with you".
 */
export function buildSections(pages: readonly PageSummary[], me: string | undefined): Sections {
  const known = new Set(pages.map((page) => page.id));
  const children = new Map<string, PageSummary[]>();
  const roots: PageSummary[] = [];
  for (const page of pages) {
    const parent = page.parent ?? undefined;
    if (parent !== undefined && known.has(parent)) {
      children.set(parent, [...(children.get(parent) ?? []), page]);
    } else {
      roots.push(page);
    }
  }
  const node = (page: PageSummary): TreeNode => ({
    page,
    children: (children.get(page.id) ?? []).toSorted(byPosition).map(node),
  });
  const sections: Sections = { workspace: [], private: [], shared: [] };
  for (const root of roots.toSorted(byPosition)) {
    const orphan = root.parent !== null && root.parent !== undefined;
    if (orphan) {
      sections.shared.push(node(root));
    } else if (root.place === "workspace") {
      sections.workspace.push(node(root));
    } else if (root.created_by === me) {
      sections.private.push(node(root));
    } else {
      sections.shared.push(node(root));
    }
  }
  return sections;
}

/** A row of the sidebar. */
export interface Row {
  page: PageSummary;
  depth: number;
  /** Open, closed, or `undefined` for a page with no sub-pages. */
  expanded: boolean | undefined;
}

/** The rows of a group, with the sub-pages of the pages in `open` shown. */
export function rowsOf(nodes: readonly TreeNode[], open: ReadonlySet<string>, depth = 0): Row[] {
  return nodes.flatMap((node) => {
    const hasChildren = node.page.has_children || node.children.length > 0;
    const expanded = hasChildren ? open.has(node.page.id) : undefined;
    return [
      { page: node.page, depth, expanded },
      ...(expanded === true ? rowsOf(node.children, open, depth + 1) : []),
    ];
  });
}

/** Every page id under `node`. */
function below(node: TreeNode): string[] {
  return node.children.flatMap((child) => [child.page.id, ...below(child)]);
}

/** Whether `candidate` is `page` itself or somewhere under it. */
export function isWithin(
  sections: Sections,
  page: string,
  candidate: string | null | undefined,
): boolean {
  if (candidate === null || candidate === undefined) {
    return false;
  }
  const find = (nodes: readonly TreeNode[]): TreeNode | undefined => {
    for (const node of nodes) {
      if (node.page.id === page) {
        return node;
      }
      const inside = find(node.children);
      if (inside !== undefined) {
        return inside;
      }
    }
    return undefined;
  };
  const node =
    find(sections.workspace) ?? find(sections.private) ?? find(sections.shared) ?? undefined;
  return node !== undefined && (node.page.id === candidate || below(node).includes(candidate));
}

/** Where a dragged page goes. */
export interface Drop {
  parent: string | null;
  place: "private" | "workspace";
  position: string;
}

/** Which part of a row the pointer is over: the top quarter, the middle, the bottom quarter. */
export function zoneOf(offset: number, height: number): "before" | "inside" | "after" {
  if (height <= 0) {
    return "inside";
  }
  const ratio = offset / height;
  if (ratio < 0.25) {
    return "before";
  }
  return ratio > 0.75 ? "after" : "inside";
}

/**
 * The parent, place and position for dropping on `target` in `zone`, among
 * `siblings` (the target's own siblings for before and after, sorted) or the
 * target's children for inside.
 */
export function dropOn(
  target: PageSummary,
  zone: "before" | "inside" | "after",
  siblings: readonly PageSummary[],
  children: readonly PageSummary[],
  dragged: string,
): Drop {
  if (zone === "inside") {
    const last = children.findLast((child) => child.id !== dragged);
    return {
      parent: target.id,
      place: target.place,
      position: generateKeyBetween(last?.position ?? null, null),
    };
  }
  const others = siblings.filter((sibling) => sibling.id !== dragged);
  const at = others.findIndex((sibling) => sibling.id === target.id);
  const before = zone === "before" ? others[at - 1] : target;
  const after = zone === "before" ? target : others[at + 1];
  return {
    parent: target.parent ?? null,
    place: target.place,
    position: generateKeyBetween(before?.position ?? null, after?.position ?? null),
  };
}

/** The key after the last sibling's, for a page made at the end of a list. */
export function endOf(siblings: readonly PageSummary[]): string {
  return generateKeyBetween(siblings.toSorted(byPosition).at(-1)?.position ?? null, null);
}
