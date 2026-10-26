import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Banner,
  Button,
  ButtonLink,
  Checkbox,
  Dialog,
  Panel,
  Select,
  SkeletonList,
  Text,
} from "~/lib/ui";
import { useMembers } from "~/modules/workspaces";

import { choiceOf, importRequest, importTree, toggle, allNodes, type ImportNode } from "../notion";
import type { UseNotionSource } from "../notion-source";

import { useNotionActions } from "./use-notion-actions";

/** Most pages one import takes (the server's cap). */
export const MAX_IMPORT = 100;

const INDENT = ["pl-0", "pl-6", "pl-12", "pl-12"] as const;

export interface ImportFromNotionProps {
  workspaceId: string;
  open: boolean;
  onClose: () => void;
  useSource: UseNotionSource;
  /** Settings → Mailboxes, where Notion is connected and reconnected. */
  settingsHref: string;
  /** The pages were made: the first of them. */
  onImported: (firstPage: string | undefined) => void;
}

/**
 * "Import from Notion": the pages the member shared with Campbooks in
 * Notion's own screen, as a tree with sub-page counts; ticking a page takes
 * its sub-pages along. Blocks Campbooks lacks come across as "Kept in
 * Notion" boxes (said before importing).
 */
export function ImportFromNotion({
  workspaceId,
  open,
  onClose,
  useSource,
  settingsHref,
  onImported,
}: ImportFromNotionProps) {
  const { t } = useTranslation("pages");
  const members = useMembers(workspaceId);
  const actions = useNotionActions(workspaceId);
  const [chosen, setChosen] = useState<string | undefined>();
  const [ticked, setTicked] = useState<ReadonlySet<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const source = useSource(workspaceId, open ? chosen : undefined);

  const connections = source.connections;
  // The first connection that works, once they are known (state adjusted
  // while rendering, which React allows when it is guarded like this).
  const firstWorking =
    connections === undefined
      ? undefined
      : (connections.find((c) => !c.needsReauth) ?? connections[0]);
  if (open && chosen === undefined && firstWorking !== undefined) {
    setChosen(firstWorking.id);
  }

  const tree = importTree(source.list.pages);
  const choice = choiceOf(tree, ticked);
  const request = importRequest(tree, ticked);
  const count = choice.included.size;
  const connection = connections?.find((candidate) => candidate.id === chosen);
  const me = members.data?.find((member) => member.you);
  const who = [me?.display_name ?? me?.email, connection?.name].filter(Boolean).join(" · ");

  const close = () => {
    setTicked(new Set());
    setFailed(false);
    onClose();
  };
  const submit = () => {
    if (chosen === undefined || request.length === 0) {
      return;
    }
    setBusy(true);
    setFailed(false);
    void actions.importPages({ connection: chosen, pages: request }, count).then((imported) => {
      setBusy(false);
      if (imported === undefined) {
        setFailed(true);
        return;
      }
      setTicked(new Set());
      onClose();
      onImported(imported.pages[0]?.page);
    });
  };

  const ready = source.list.status === "ready" && connection?.needsReauth !== true;
  return (
    <Dialog
      open={open}
      onClose={close}
      title={t("notion.import.title")}
      description={who === "" ? undefined : t("notion.import.connectedAs", { who })}
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            {t("notion.import.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={!ready || request.length === 0 || request.length > MAX_IMPORT || busy}
            onClick={submit}
          >
            {count === 0 ? t("notion.import.submitNone") : t("notion.import.submit", { count })}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <Text tone="secondary">{t("notion.import.lead")}</Text>
        {connections !== undefined && connections.length > 1 ? (
          <Select
            label={t("notion.import.account")}
            options={connections.map((c) => ({ value: c.id, label: c.name }))}
            value={chosen ?? ""}
            onChange={(next) => {
              setChosen(next);
              setTicked(new Set());
            }}
          />
        ) : null}
        <Body
          connectionsKnown={
            connections !== undefined && (connections.length === 0 || chosen !== undefined)
          }
          hasConnection={connection !== undefined}
          needsReauth={connection?.needsReauth === true}
          source={source}
          tree={tree}
          included={choice.included}
          ticked={ticked}
          settingsHref={settingsHref}
          onToggle={(id) => {
            setTicked((current) => toggle(tree, current, id));
          }}
        />
        {request.length > MAX_IMPORT ? (
          <Banner
            tone="danger"
            role="alert"
            title={t("notion.import.tooMany", { max: MAX_IMPORT })}
          />
        ) : null}
        {failed ? <Banner tone="danger" role="alert" title={t("notion.import.failed")} /> : null}
        <Panel>
          <Text tone="secondary" size="sm">
            {t("notion.import.info")}
          </Text>
        </Panel>
      </div>
    </Dialog>
  );
}

interface BodyProps {
  connectionsKnown: boolean;
  hasConnection: boolean;
  needsReauth: boolean;
  source: ReturnType<UseNotionSource>;
  tree: readonly ImportNode[];
  included: ReadonlySet<string>;
  ticked: ReadonlySet<string>;
  settingsHref: string;
  onToggle: (id: string) => void;
}

function Body({
  connectionsKnown,
  hasConnection,
  needsReauth,
  source,
  tree,
  included,
  ticked,
  settingsHref,
  onToggle,
}: BodyProps) {
  const { t } = useTranslation("pages");
  if (!connectionsKnown || (hasConnection && source.list.status === "loading")) {
    return <SkeletonList rows={4} />;
  }
  if (!hasConnection) {
    return (
      <div className="flex flex-col items-start gap-2">
        <strong>{t("notion.import.noConnection.title")}</strong>
        <Text tone="secondary">{t("notion.import.noConnection.lead")}</Text>
        <ButtonLink variant="secondary" href={settingsHref}>
          {t("notion.import.noConnection.settings")}
        </ButtonLink>
      </div>
    );
  }
  if (needsReauth || source.list.error === "needs_reauth") {
    return (
      <div className="flex flex-col items-start gap-2">
        <strong>{t("notion.import.errors.needs_reauth.title")}</strong>
        <Text tone="secondary">{t("notion.import.errors.needs_reauth.lead")}</Text>
        <ButtonLink variant="secondary" href={settingsHref}>
          {t("notion.import.noConnection.settings")}
        </ButtonLink>
      </div>
    );
  }
  if (source.list.status === "error" && source.list.error !== undefined) {
    return (
      <div className="flex flex-col items-start gap-2" role="alert">
        <strong>{t(`notion.import.errors.${source.list.error}.title`)}</strong>
        <Text tone="secondary">{t(`notion.import.errors.${source.list.error}.lead`)}</Text>
        <Button variant="secondary" onClick={source.list.retry}>
          {t("notion.import.retry")}
        </Button>
      </div>
    );
  }
  if (tree.length === 0) {
    return (
      <div className="flex flex-col gap-1">
        <strong>{t("notion.import.empty.title")}</strong>
        <Text tone="secondary">{t("notion.import.empty.lead")}</Text>
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-1" role="group" aria-label={t("notion.import.listLabel")}>
      {allNodes(tree).map(({ page, depth }) => {
        const came = included.has(page.notion_page) && !ticked.has(page.notion_page);
        return (
          <div key={page.notion_page} className={INDENT[Math.min(depth, INDENT.length - 1)]}>
            <Checkbox
              label={
                <span className="break-words">
                  {page.title === "" ? t("untitled") : page.title}
                </span>
              }
              hint={
                page.linked
                  ? t("notion.import.linked")
                  : page.sub_pages > 0
                    ? t("notion.import.subPages", { count: page.sub_pages })
                    : undefined
              }
              checked={included.has(page.notion_page) || page.linked}
              disabled={page.linked || came}
              onChange={() => {
                onToggle(page.notion_page);
              }}
            />
          </div>
        );
      })}
      {source.list.truncated ? (
        <Text tone="tertiary" size="sm">
          {t("notion.import.truncated")}
        </Text>
      ) : null}
    </div>
  );
}
