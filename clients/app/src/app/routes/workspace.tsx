import { useQueryClient } from "@tanstack/react-query";
import { Outlet, useLocation, useNavigate, useParams } from "@tanstack/react-router";
import { useCallback, useEffect, useState, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { createWebChangeSource, followsChangesLive, subscribeToChanges } from "~/lib/api";
import { ScoutChatProvider } from "~/lib/scout-chat";
import { useShortcut, useShortcuts } from "~/lib/shortcuts";
import {
  Button,
  ChipLink,
  ChipRow,
  FocusPage,
  IconButton,
  initialsOf,
  NavItem,
  PhoneHeader,
  PhoneSubnav,
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
import { ScoutChat } from "~/modules/assistant";
import { useAttentionCount } from "~/modules/attention";
import { StreamNav, isPlace, streamPath, type Place } from "~/modules/conversations";
import { useTaskCount } from "~/modules/tasks";
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
 * only for now: the native source is not built yet, see
 * `followsChangesLive`, and there is nothing to follow where `EventSource`
 * is missing).
 */
function useWorkspaceChanges(workspaceId: string) {
  const queryClient = useQueryClient();
  useEffect(() => {
    if (!followsChangesLive()) {
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

/** Settings' sections, in order: where each lives under `/w/$workspaceId/settings`. */
const SETTINGS_SECTIONS = [
  { id: "members", path: "members" },
  { id: "mailboxes", path: "mailboxes" },
  { id: "waysIn", path: "sign-in" },
  { id: "memory", path: "memory" },
] as const;

function WorkspaceFrame({ workspace, all }: { workspace: Workspace; all: Workspace[] }) {
  const { t } = useTranslation();
  const { t: tNav } = useTranslation("shortcuts");
  const { t: tSearch } = useTranslation("search");
  const { t: tWorkspaces } = useTranslation("workspaces");
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { registry } = useShortcuts();
  const theme = useTheme();
  const link = useInAppLink();
  const [switching, setSwitching] = useState(false);
  // Scout's chat: one panel, about one conversation at a time; the focus
  // goes back to what opened it when it closes.
  const [chat, setChat] = useState<{
    conversationId: string;
    from: HTMLElement | null | undefined;
  }>();
  const openChat = useCallback((conversationId: string, from?: HTMLElement | null) => {
    setChat({ conversationId, from });
  }, []);
  const closeChat = () => {
    const from = chat?.from;
    setChat(undefined);
    // After the panel is gone, so its focus trap gives nothing back first.
    requestAnimationFrame(() => {
      if (from?.isConnected === true) {
        from.focus();
      }
    });
  };
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
  const inSettings = pathname.startsWith(`${base}/settings`);
  const inInbox = pathname.startsWith(`${base}/inbox`);
  const inSearch = pathname.startsWith(`${base}/search`);
  const inTasks = pathname.startsWith(`${base}/tasks`);
  const atHome = !inSettings && !inInbox && !inSearch && !inTasks;
  const needsYou = useAttentionCount(workspace.id);
  const openTasks = useTaskCount(workspace.id);
  const tasksPath = `${base}/tasks`;
  const inboxPath = streamPath(workspace.id, "primary");
  const searchPath = `${base}/search`;
  // The stream, view or Drafts on screen (`/inbox/sent`…): these are places, not streams.
  const place = /\/inbox\/([^/]+)/u.exec(pathname)?.[1];
  const openStream: Place | undefined = place !== undefined && isPlace(place) ? place : undefined;

  useShortcut("workspace.switch", () => {
    setSwitching((open) => !open);
  });
  useShortcut("nav.home", () => {
    void navigate({ to: base });
  });
  useShortcut("nav.inbox", () => {
    void navigate({ to: inboxPath });
  });
  useShortcut("nav.tasks", () => {
    void navigate({ to: tasksPath });
  });
  // N T, from anywhere: Tasks, with "Add a task…" focused.
  useShortcut("tasks.create", () => {
    void navigate({ to: tasksPath, search: { add: Date.now() } });
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
  const sections = SETTINGS_SECTIONS.map((section) => {
    const href = `${base}/settings/${section.path}`;
    return {
      ...section,
      href,
      label: t(`settings.${section.id}`),
      current: pathname === href || pathname.startsWith(`${href}/`),
    };
  });

  return (
    <ScoutChatProvider open={openChat}>
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
          <AccountMenu compact waysIn={link(waysInPath)} apps={link(appsPath)} />
        </PhoneHeader>
        {inSettings ? (
          <PhoneSubnav label={tNav("nav.settings")}>
            <ChipRow label={t("settings.sections")}>
              {sections.map((section) => (
                <ChipLink key={section.id} current={section.current} {...link(section.href)}>
                  {section.label}
                </ChipLink>
              ))}
            </ChipRow>
          </PhoneSubnav>
        ) : null}
        <Sidebar>
          <WorkspaceSwitcher mark={mark} name={workspace.name} kind={kind} onClick={openSwitcher} />
          <SearchButton
            onClick={() => {
              registry.run("command.open");
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
            label={tNav("nav.tasks")}
            icon="tasks"
            shortcut="nav.tasks"
            current={inTasks}
            count={openTasks}
            {...link(tasksPath)}
          />
          <NavItem
            label={tNav("nav.settings")}
            icon="settings"
            shortcut="nav.settings"
            current={inSettings && !sections.some((section) => section.current)}
            {...link(membersPath)}
          />
          {inSettings
            ? sections.map((section) => (
                <NavItem
                  key={section.id}
                  sub
                  label={section.label}
                  current={section.current}
                  {...link(section.href)}
                />
              ))
            : null}
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
            <AccountMenu waysIn={link(waysInPath)} apps={link(appsPath)} />
          </SidebarFooter>
        </Sidebar>
        <Outlet />
        {chat === undefined ? null : (
          <ScoutChat
            key={chat.conversationId}
            workspaceId={workspace.id}
            conversationId={chat.conversationId}
            onClose={closeChat}
          />
        )}
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
            id: "tasks",
            label: tNav("nav.tasks"),
            icon: "tasks",
            current: inTasks,
            ...(openTasks === undefined ? {} : { badge: openTasks }),
            ...link(tasksPath),
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
    </ScoutChatProvider>
  );
}
