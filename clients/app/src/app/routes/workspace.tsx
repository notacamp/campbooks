import { useQueryClient } from "@tanstack/react-query";
import { Outlet, useLocation, useNavigate, useParams } from "@tanstack/react-router";
import { useEffect, useState, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { createWebChangeSource, subscribeToChanges } from "~/lib/api";
import { isTauri } from "~/lib/platform";
import { useShortcut, useShortcuts } from "~/lib/shortcuts";
import {
  Button,
  FocusPage,
  IconButton,
  initialsOf,
  NavItem,
  PhoneHeader,
  SearchButton,
  Shell,
  Sidebar,
  SidebarFooter,
  Spacer,
  TabBar,
  useTheme,
  Visor,
  WorkspaceMenu,
  WorkspaceSwitcher,
} from "~/lib/ui";
import { useAttentionCount } from "~/modules/attention";
import { StreamNav, streamPath } from "~/modules/conversations";
import { useWorkspaces, type Workspace } from "~/modules/workspaces";

import { AccountMenu } from "../account/AccountMenu";

/** An in-app link for a nav item: a real href, followed without a reload. */
function useInAppLink() {
  const navigate = useNavigate();
  return (href: string) => ({
    href,
    onClick: (event: MouseEvent<HTMLElement>) => {
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) {
        return;
      }
      event.preventDefault();
      void navigate({ to: href });
    },
  });
}

/**
 * Keeps the open workspace's cached queries fresh from its change feed (web
 * only for now: the native source is not built yet, and there is nothing to
 * follow where `EventSource` is missing).
 */
function useWorkspaceChanges(workspaceId: string) {
  const queryClient = useQueryClient();
  useEffect(() => {
    if (isTauri() || typeof EventSource === "undefined") {
      return;
    }
    return subscribeToChanges({
      queryClient,
      workspace: workspaceId,
      source: createWebChangeSource(),
    });
  }, [queryClient, workspaceId]);
}

/**
 * `/w/$workspaceId`: the frame of a workspace (sidebar, or phone header and
 * tab bar), its accent, the switcher (⌘⇧W) and the account menu.
 */
export function WorkspaceLayout() {
  const { t } = useTranslation();
  const workspaces = useWorkspaces();
  const { workspaceId } = useParams({ strict: false });
  const navigate = useNavigate();
  const workspace = workspaces.data?.find((candidate) => candidate.id === workspaceId);

  if (workspaces.isPending) {
    return (
      <FocusPage
        leading={<Visor state="thinking" size="xl" announce />}
        title={t("gate.opening")}
      />
    );
  }
  if (workspace === undefined) {
    const failed = workspaces.isError;
    return (
      <FocusPage
        leading={<Visor state={failed ? "thinking" : "unsure"} size="xl" />}
        title={failed ? t("workspace.loadErrorTitle") : t("workspace.unavailableTitle")}
        lead={failed ? t("workspace.loadErrorLead") : t("workspace.unavailableLead")}
        actions={
          <Button
            variant="primary"
            size="lg"
            onClick={() => {
              if (failed) {
                void workspaces.refetch();
              } else {
                void navigate({ to: "/" });
              }
            }}
          >
            {failed ? t("gate.retry") : t("workspace.openMine")}
          </Button>
        }
      />
    );
  }
  return <WorkspaceFrame workspace={workspace} all={workspaces.data ?? []} />;
}

function WorkspaceFrame({ workspace, all }: { workspace: Workspace; all: Workspace[] }) {
  const { t: tNav } = useTranslation("shortcuts");
  const { t: tSearch } = useTranslation("search");
  const { t: tWorkspaces } = useTranslation("workspaces");
  const { t: tShell } = useTranslation();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { registry } = useShortcuts();
  const theme = useTheme();
  const link = useInAppLink();
  const [switching, setSwitching] = useState(false);
  useWorkspaceChanges(workspace.id);

  // The server's accent is the source of truth; the device remembers it so
  // the next load paints it before any code runs.
  const { accent, setAccent } = theme;
  useEffect(() => {
    if (accent !== workspace.accent) {
      setAccent(workspace.accent);
    }
  }, [accent, setAccent, workspace.accent]);

  const base = `/w/${workspace.id}`;
  const membersPath = `${base}/settings/members`;
  const waysInPath = `${base}/settings/sign-in`;
  const appsPath = `${base}/settings/apps`;
  const memoryPath = `${base}/settings/memory`;
  const inMemory = pathname.startsWith(memoryPath);
  const inSettings = pathname.startsWith(`${base}/settings`);
  const inInbox = pathname.startsWith(`${base}/inbox`);
  const inSearch = pathname.startsWith(`${base}/search`);
  const atHome = !inSettings && !inInbox && !inSearch;
  const needsYou = useAttentionCount(workspace.id);
  const inboxPath = streamPath(workspace.id, "primary");
  const searchPath = `${base}/search`;
  const openStream = /\/inbox\/(primary|updates|receipts|newsletters)\b/u.exec(pathname)?.[1] as
    "primary" | "updates" | "receipts" | "newsletters" | undefined;

  useShortcut("workspace.switch", () => {
    setSwitching((open) => !open);
  });
  useShortcut("nav.home", () => {
    void navigate({ to: base });
  });
  useShortcut("nav.inbox", () => {
    void navigate({ to: inboxPath });
  });
  useShortcut("nav.settings", () => {
    void navigate({ to: membersPath });
  });
  useShortcut("workspaces.create", () => {
    void navigate({ to: "/welcome" });
  });

  const kind = tWorkspaces(`kind.${workspace.kind}`);
  const mark = initialsOf(workspace.name);
  const openSwitcher = () => {
    setSwitching(true);
  };

  return (
    <>
      <Shell>
        <PhoneHeader>
          <WorkspaceSwitcher
            compact
            mark={mark}
            name={workspace.name}
            kind={kind}
            onClick={openSwitcher}
          />
          <Spacer />
          <IconButton
            size="lg"
            icon="search"
            label={tSearch("shortcuts.open")}
            shortcut="search.open"
            onClick={() => {
              void navigate({ to: searchPath });
            }}
          />
          <AccountMenu
            compact
            waysIn={link(waysInPath)}
            apps={link(appsPath)}
            memory={link(memoryPath)}
          />
        </PhoneHeader>
        <Sidebar>
          <WorkspaceSwitcher mark={mark} name={workspace.name} kind={kind} onClick={openSwitcher} />
          <SearchButton
            onClick={() => {
              registry.run("command.open");
            }}
          />
          <NavItem
            label={tSearch("shortcuts.open")}
            icon="search"
            shortcut="search.open"
            current={inSearch}
            onClick={() => {
              registry.run("search.open");
            }}
          />
          <NavItem
            label={tNav("nav.home")}
            icon="home"
            shortcut="nav.home"
            current={atHome}
            count={needsYou}
            {...link(base)}
          />
          <NavItem
            label={tNav("nav.inbox")}
            icon="inbox"
            shortcut="nav.inbox"
            current={inInbox && openStream === undefined}
            {...link(inboxPath)}
          />
          <StreamNav workspaceId={workspace.id} current={openStream} />
          <NavItem
            label={tShell("account.memory")}
            icon="star"
            current={inMemory}
            {...link(memoryPath)}
          />
          <NavItem
            label={tNav("nav.settings")}
            icon="settings"
            shortcut="nav.settings"
            current={inSettings && !inMemory}
            {...link(membersPath)}
          />
          <SidebarFooter>
            <Button
              variant="ghost"
              icon="keyboard"
              shortcut="sheet.open"
              onClick={() => {
                registry.run("sheet.open");
              }}
            >
              {tNav("sheet.open")}
            </Button>
            <AccountMenu
              waysIn={link(waysInPath)}
              apps={link(appsPath)}
              memory={link(memoryPath)}
            />
          </SidebarFooter>
        </Sidebar>
        <Outlet />
      </Shell>
      <TabBar
        phoneOnly
        items={[
          {
            id: "home",
            label: tNav("nav.home"),
            icon: "home",
            current: atHome,
            ...(needsYou === undefined ? {} : { badge: needsYou }),
            ...link(base),
          },
          {
            id: "inbox",
            label: tNav("nav.inbox"),
            icon: "inbox",
            current: inInbox,
            ...link(inboxPath),
          },
          {
            id: "settings",
            label: tNav("nav.settings"),
            icon: "settings",
            current: inSettings,
            ...link(membersPath),
          },
        ]}
      />
      <WorkspaceMenu
        open={switching}
        onClose={() => {
          setSwitching(false);
        }}
        currentId={workspace.id}
        workspaces={all.map((candidate) => ({
          id: candidate.id,
          name: candidate.name,
          mark: initialsOf(candidate.name),
          kind: tWorkspaces(`kind.${candidate.kind}`),
          accent: candidate.accent,
        }))}
        onSelect={(id) => {
          void navigate({ to: "/w/$workspaceId", params: { workspaceId: id } });
        }}
        onCreate={() => {
          void navigate({ to: "/welcome" });
        }}
        createShortcut="workspaces.create"
      />
    </>
  );
}
