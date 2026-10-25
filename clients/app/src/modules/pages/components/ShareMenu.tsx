import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Avatar, Button, Popover, PopoverSection, Select, Spacer, Text } from "~/lib/ui";
import { useMembers, type Member } from "~/modules/workspaces";

import type { PageRole, PageView } from "../types";

import type { usePageActions } from "./use-page-actions";

const ROLES = ["reader", "writer", "manager"] as const;

const nameOf = (member: Member | undefined) =>
  member === undefined ? "" : (member.display_name ?? member.email ?? "");

export interface ShareMenuProps {
  workspaceId: string;
  page: PageView;
  actions: ReturnType<typeof usePageActions>;
}

/**
 * Who a page is shared with: its place, the members it is shared with and
 * their role, and (for someone who may share it) adding and removing them.
 * A mention keeps its own access, and says so.
 */
export function ShareMenu({ workspaceId, page, actions }: ShareMenuProps) {
  const { t } = useTranslation("pages");
  const members = useMembers(workspaceId).data ?? [];
  const canShare = page.role === "manager";
  const shared = new Set(page.grants.map((grant) => grant.member));
  const candidates = members.filter(
    (member) => !member.you && !shared.has(member.user_id) && member.user_id !== page.created_by,
  );
  const [who, setWho] = useState("");
  const [role, setRole] = useState<PageRole>("writer");
  const chosen = candidates.find((member) => member.user_id === who) ?? candidates[0];
  const roleOptions = ROLES.map((value) => ({ value, label: t(`share.role.${value}`) }));
  const creator = members.find((member) => member.user_id === page.created_by);

  return (
    <Popover
      label={t("share.title", { title: page.title === "" ? t("untitled") : page.title })}
      placement="bottom-end"
      renderTrigger={(trigger) => (
        <Button {...trigger} icon="people">
          {t("share.button")}
        </Button>
      )}
    >
      <PopoverSection
        title={t(page.place === "workspace" ? "share.placeWorkspace" : "share.placePrivate")}
      >
        <Text tone="tertiary" size="sm">
          {t(page.place === "workspace" ? "share.placeWorkspaceNote" : "share.placePrivateNote")}
        </Text>
      </PopoverSection>
      <PopoverSection title={t("share.people")}>
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <Avatar name={nameOf(creator)} size="sm" />
            <Text>{nameOf(creator) === "" ? t("share.someone") : nameOf(creator)}</Text>
            <Spacer />
            <Text tone="tertiary" size="sm">
              {t("share.owner")}
            </Text>
          </div>
          {page.grants.map((grant) => {
            const member = members.find((candidate) => candidate.user_id === grant.member);
            const name = nameOf(member) === "" ? t("share.someone") : nameOf(member);
            return (
              <div key={grant.member} className="flex flex-wrap items-center gap-2">
                <Avatar name={name} size="sm" />
                <Text>{name}</Text>
                <Spacer />
                {canShare ? (
                  <>
                    <Select
                      label={t("share.accessOf", { name })}
                      hideLabel
                      options={roleOptions}
                      value={grant.role}
                      onChange={(next) => {
                        void actions.share(page, grant.member, next, grant.role, name);
                      }}
                    />
                    <Button
                      variant="ghost"
                      onClick={() => {
                        void actions.unshare(page, grant.member, grant.role, name);
                      }}
                    >
                      {t("share.remove", { name })}
                    </Button>
                  </>
                ) : (
                  <Text tone="tertiary" size="sm">
                    {t(`share.role.${grant.role}`)}
                  </Text>
                )}
              </div>
            );
          })}
        </div>
      </PopoverSection>
      {canShare && chosen !== undefined ? (
        <PopoverSection title={t("share.add")}>
          <div className="flex flex-col gap-2">
            <Select
              label={t("share.who")}
              options={candidates.map((member) => ({
                value: member.user_id,
                label: nameOf(member),
              }))}
              value={chosen.user_id}
              onChange={setWho}
            />
            <Select
              label={t("share.access")}
              options={roleOptions}
              value={role}
              onChange={setRole}
            />
            <Button
              variant="primary"
              onClick={() => {
                void actions.share(page, chosen.user_id, role, undefined, nameOf(chosen));
              }}
            >
              {t("share.invite")}
            </Button>
          </div>
        </PopoverSection>
      ) : null}
      <PopoverSection>
        <Text tone="tertiary" size="sm">
          {t("share.mentions")}
        </Text>
      </PopoverSection>
    </Popover>
  );
}
