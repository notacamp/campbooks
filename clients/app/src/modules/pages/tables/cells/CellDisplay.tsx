import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { browserUrl } from "~/lib/api";
import {
  Avatar,
  CellLine,
  CellLink,
  CellRow,
  MentionChip,
  OptionTag,
  type MentionKind,
} from "~/lib/ui";

import { useRecordLabel } from "../../api/records";
import { isMentionType, type MentionType } from "../../types";
import { useRowsById } from "../api";
import {
  colorOf,
  filesOf,
  formatDate,
  formatInstant,
  formatNumber,
  hrefOf,
  idsOf,
  isRecord,
  linksOf,
  optionById,
  safeHref,
  type LinkRef,
} from "../model";
import type { PropertyView, RowView } from "../types";

import { memberName, type CellContext } from "./context";

const MENTION_KINDS = {
  "workspaces.member": "person",
  person: "person",
  "conversations.conversation": "thread",
  "tasks.task": "task",
  "schedule.event": "event",
  "pages.page": "page",
} as const satisfies Record<MentionType, MentionKind>;

/** A Campbooks record a cell links to: its own name for the viewer, or "Something you can't open". */
export function LinkChip({ ctx, link }: { ctx: CellContext; link: LinkRef }) {
  const { t } = useTranslation("pages");
  const known = isMentionType(link.type) ? link.type : undefined;
  const label = useRecordLabel(ctx.workspaceId, known ?? "person", link.id);
  const state = known === undefined ? "closed" : label.state;
  const href = known === undefined ? undefined : label.href;
  return (
    <MentionChip
      kind={known === undefined ? "page" : MENTION_KINDS[known]}
      state={state}
      label={state === "closed" ? t("mention.closed") : label.label}
      href={href}
      onOpen={(event: MouseEvent<HTMLAnchorElement>) => {
        if (href !== undefined && !event.metaKey && !event.ctrlKey && event.button === 0) {
          event.preventDefault();
          ctx.open(href);
        }
      }}
    />
  );
}

/** Rows of another table a relation cell names, by their titles (the ones the viewer can read). */
function RelationChips({
  ctx,
  property,
  ids,
}: {
  ctx: CellContext;
  property: PropertyView;
  ids: readonly string[];
}) {
  const { t } = useTranslation("pages");
  const table =
    isRecord(property.config) && typeof property.config["table"] === "string"
      ? property.config["table"]
      : undefined;
  const rows = useRowsById(ctx.workspaceId, table, ids);
  return (
    <CellRow>
      {ids.map((id) => {
        const row = rows.data?.rows.find((candidate) => candidate.id === id);
        const state = rows.isPending ? "loading" : row === undefined ? "closed" : "ready";
        const href = `/w/${ctx.workspaceId}/pages/${id}`;
        return (
          <MentionChip
            key={id}
            kind="page"
            state={state}
            label={
              state === "closed"
                ? t("mention.closed")
                : row === undefined || row.title === ""
                  ? t("untitled")
                  : row.title
            }
            href={state === "ready" ? href : undefined}
            onOpen={(event) => {
              if (!event.metaKey && !event.ctrlKey && event.button === 0) {
                event.preventDefault();
                ctx.open(href);
              }
            }}
          />
        );
      })}
    </CellRow>
  );
}

export interface CellDisplayProps {
  ctx: CellContext;
  property: PropertyView;
  row: RowView;
  value: unknown;
}

/** What a cell shows, read-only: one function for the grid, the board's cards and the row's page. */
export function CellDisplay({ ctx, property, row, value }: CellDisplayProps) {
  const { t } = useTranslation("pages");
  switch (property.type) {
    case "title": {
      return <CellLine>{row.title === "" ? t("untitled") : row.title}</CellLine>;
    }
    case "text": {
      return <TextRuns value={value} />;
    }
    case "number": {
      const shown = formatNumber(value, property, ctx.locale);
      return shown === "" ? null : <CellLine end>{shown}</CellLine>;
    }
    case "select":
    case "status": {
      const option = optionById(property, idsOf(value)[0]);
      return option === undefined ? null : (
        <OptionTag color={colorOf(option.color)}>{option.name}</OptionTag>
      );
    }
    case "multi_select": {
      return (
        <CellRow>
          {idsOf(value).map((id) => {
            const option = optionById(property, id);
            return option === undefined ? null : (
              <OptionTag key={id} color={colorOf(option.color)}>
                {option.name}
              </OptionTag>
            );
          })}
        </CellRow>
      );
    }
    case "date": {
      const shown = formatDate(value, ctx.locale);
      return shown === "" ? null : <CellLine>{shown}</CellLine>;
    }
    case "checkbox": {
      return (
        <input
          type="checkbox"
          readOnly
          tabIndex={-1}
          checked={value === true}
          aria-label={property.name}
        />
      );
    }
    case "person": {
      return (
        <CellRow>
          {idsOf(value).map((id) => {
            const name = memberName(ctx.members, id) ?? t("tables.cell.someone");
            return (
              <span key={id} className="inline-flex items-center gap-1">
                <Avatar name={name} size="sm" />
                <CellLine>{name}</CellLine>
              </span>
            );
          })}
        </CellRow>
      );
    }
    case "files": {
      return (
        <CellRow>
          {filesOf(value).map((file) => {
            const href = browserUrl(
              `/api/v1/w/${ctx.workspaceId}/pages/${row.id}/files/${file.hash}`,
            );
            return href === undefined ? (
              <CellLine key={file.hash}>{file.name}</CellLine>
            ) : (
              <CellLink key={file.hash} href={href}>
                {file.name}
              </CellLink>
            );
          })}
        </CellRow>
      );
    }
    case "url":
    case "email": {
      const content = typeof value === "string" ? value : "";
      const href = hrefOf(property.type, content);
      return content === "" ? null : href === undefined ? (
        <CellLine>{content}</CellLine>
      ) : (
        <CellLink href={href}>{content}</CellLink>
      );
    }
    case "phone": {
      return typeof value === "string" && value !== "" ? <CellLine>{value}</CellLine> : null;
    }
    case "relation": {
      const ids = idsOf(value);
      return ids.length === 0 ? null : <RelationChips ctx={ctx} property={property} ids={ids} />;
    }
    case "campbooks_link": {
      return (
        <CellRow>
          {linksOf(value).map((link) => (
            <LinkChip key={`${link.type}:${link.id}`} ctx={ctx} link={link} />
          ))}
        </CellRow>
      );
    }
    case "created":
    case "edited": {
      const who = isRecord(property.config) && property.config["of"] === "who";
      const at = property.type === "created" ? row.created_at : row.edited_at;
      const by = property.type === "created" ? row.created_by : row.edited_by;
      return (
        <CellLine>
          {who
            ? (memberName(ctx.members, by) ?? t("tables.cell.someone"))
            : formatInstant(at, ctx.locale)}
        </CellLine>
      );
    }
    case "unique_id": {
      const prefix =
        isRecord(property.config) && typeof property.config["prefix"] === "string"
          ? property.config["prefix"]
          : "";
      return (
        <CellLine>
          {prefix === "" ? String(row.number) : `${prefix}-${String(row.number)}`}
        </CellLine>
      );
    }
    default: {
      // A property kept in Notion: shown as text when it came as text, never edited.
      return typeof value === "string" ? (
        <CellLine>{value}</CellLine>
      ) : (
        <CellLine>{t("tables.cell.keptInNotion")}</CellLine>
      );
    }
  }
}

/** A text cell's runs; a run with a link shows as one only when its address is safe. */
function TextRuns({ value }: { value: unknown }) {
  if (!Array.isArray(value)) {
    return null;
  }
  const runs = value as unknown[];
  if (runs.length === 0) {
    return null;
  }
  return (
    <CellLine>
      {runs.map((run, index) => {
        if (!isRecord(run) || typeof run["text"] !== "string") {
          return null;
        }
        const href = typeof run["href"] === "string" ? safeHref(run["href"]) : undefined;
        return href === undefined ? (
          <span key={index}>{run["text"]}</span>
        ) : (
          <a key={index} href={href} target="_blank" rel="noopener noreferrer">
            {run["text"]}
          </a>
        );
      })}
    </CellLine>
  );
}
