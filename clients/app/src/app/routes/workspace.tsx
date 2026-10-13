import { Outlet, useLocation, useNavigate, useParams } from "@tanstack/react-router";
import { useEffect, useState, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut, useShortcuts } from "~/lib/shortcuts";
import {
  Button,
  FocusPage,
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
  const { t: tWorkspaces } = useTranslation("workspaces");
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { registry } = useShortcuts();
  const theme = useTheme();
  const link = useInAppLink();
  const [switching, setSwitching] = useState(false);

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
  const inSettings = pathname.startsWith(`${base}/settings`);
  const atHome = !inSettings;

  useShortcut("workspace.switch", () => {
    setSwitching((open) => !open);
  });
  useShortcut("nav.home", () => {
    void navigate({ to: base });
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
          <AccountMenu compact />
        </PhoneHeader>
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
            {...link(base)}
          />
          <NavItem
            label={tNav("nav.settings")}
            icon="settings"
            shortcut="nav.settings"
            current={inSettings}
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
            <AccountMenu />
          </SidebarFooter>
        </Sidebar>
        <Outlet />
      </Shell>
      <TabBar
        phoneOnly
        items={[
          { id: "home", label: tNav("nav.home"), icon: "home", current: atHome, ...link(base) },
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
