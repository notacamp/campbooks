import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Avatar, Button, Heading, Text, TextField } from "~/lib/ui";
import { useMembers, type Member } from "~/modules/workspaces";

import type { TaskView } from "../api/queries";

/** A member's name as people see it: never typed, never an address if they have a name. */
export const memberName = (member: Member): string =>
  member.display_name ?? member.email ?? "";

export interface HandOffProps {
  workspaceId: string;
  task: TaskView;
  /** The caller. */
  me: string | undefined;
  onHandOff: (member: { id: string; name: string }) => void;
  onTakeBack: () => void;
}

/**
 * Hand it to someone: a filter box by teammate name, never an address. Once
 * handed off it says who has it, with a way to take it back.
 */
export function HandOff({ workspaceId, task, me, onHandOff, onTakeBack }: HandOffProps) {
  const { t: typed } = useTranslation("tasks");
  const t = typed as unknown as (key: string, options?: Record<string, unknown>) => string;
  const members = useMembers(workspaceId).data ?? [];
  const [query, setQuery] = useState("");
  const byId = new Map(members.map((member) => [member.user_id, member] as const));
  const nameOf = (id: string | null | undefined) => {
    const member = id === null || id === undefined ? undefined : byId.get(id);
    return member === undefined ? t("detail.someone") : memberName(member);
  };

  const heading = (
    <Heading level={3} size="section">
      {t("detail.hand")}
    </Heading>
  );
  // Handed to someone else by the caller: who has it, and the way back.
  if (task.handed_by !== null && task.handed_by !== undefined && task.handed_by === me) {
    return (
      <section className="flex flex-col gap-2">
        {heading}
        <div className="flex flex-wrap items-center gap-2">
          <Text>{t("row.with", { name: nameOf(task.assignee) })}</Text>
          <Button variant="link" onClick={onTakeBack}>
            {t("detail.takeBack")}
          </Button>
        </div>
      </section>
    );
  }
  // Handed to the caller: who gave it. Tasks stay with one person.
  if (task.handed_by !== null && task.handed_by !== undefined) {
    return (
      <section className="flex flex-col gap-2">
        {heading}
        <Text tone="secondary">{t("detail.handedToYou", { name: nameOf(task.handed_by) })}</Text>
      </section>
    );
  }
  const needle = query.trim().toLowerCase();
  const matches =
    needle === ""
      ? []
      : members.filter(
          (member) => member.user_id !== me && memberName(member).toLowerCase().includes(needle),
        );
  return (
    <section className="flex flex-col gap-2">
      {heading}
      <TextField
        label={t("detail.handFind")}
        hideLabel
        placeholder={t("detail.handFind")}
        autoComplete="off"
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
        }}
      />
      {needle !== "" && matches.length === 0 ? (
        <Text tone="tertiary" size="sm">
          {t("detail.handNone")}
        </Text>
      ) : null}
      {matches.length === 0 ? null : (
        <div className="flex flex-col gap-1">
          {matches.map((member) => (
            <Button
              key={member.user_id}
              variant="ghost"
              leading={<Avatar name={memberName(member)} />}
              onClick={() => {
                setQuery("");
                onHandOff({ id: member.user_id, name: memberName(member) });
              }}
            >
              {memberName(member)}
            </Button>
          ))}
        </div>
      )}
    </section>
  );
}
