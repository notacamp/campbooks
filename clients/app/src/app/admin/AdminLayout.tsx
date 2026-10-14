import { Outlet, useLocation, useNavigate } from "@tanstack/react-router";
import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { loginUrl } from "~/lib/api";
import {
  Button,
  ButtonLink,
  FocusPage,
  IconButton,
  NavItem,
  PhoneHeader,
  Shell,
  Sidebar,
  SidebarFooter,
  Spacer,
  TabBar,
  Text,
  Visor,
} from "~/lib/ui";

import { AccountMenu } from "../account/AccountMenu";
import { ADMIN_SECTIONS } from "../modules";
import { NotFound } from "../routes/not-found";

import { useAdminAccess } from "./access";

/**
 * `/admin`: the instance administration section, outside any workspace.
 * Shown to administrators only; everyone else gets the not-found page, as
 * the admin API answers them 404.
 */
export function AdminLayout() {
  const { t } = useTranslation();
  const { access, retry } = useAdminAccess();
  const { pathname } = useLocation();

  switch (access) {
    case "checking": {
      return (
        <FocusPage
          leading={<Visor state="thinking" size="xl" announce />}
          title={t("admin.opening")}
        />
      );
    }
    case "none": {
      return <NotFound />;
    }
    case "reauth": {
      return <ReauthNotice returnTo={pathname} />;
    }
    case "error": {
      return (
        <FocusPage
          leading={<Visor state="thinking" size="xl" />}
          title={t("admin.unreachableTitle")}
          lead={t("admin.unreachableLead")}
          actions={
            <Button variant="primary" size="lg" onClick={retry}>
              {t("gate.retry")}
            </Button>
          }
        />
      );
    }
    case "admin": {
      return <AdminFrame />;
    }
  }
}

/** Administration asks for a recent sign-in (1 hour): explain, then sign in again. */
function ReauthNotice({ returnTo }: { returnTo: string }) {
  const { t } = useTranslation();
  return (
    <FocusPage
      leading={<Visor state="unsure" size="xl" />}
      title={t("admin.reauthTitle")}
      lead={t("admin.reauthLead")}
      actions={
        <ButtonLink variant="primary" size="lg" href={loginUrl(returnTo, { fresh: true })}>
          {t("admin.reauthAction")}
        </ButtonLink>
      }
    />
  );
}

function AdminFrame() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const link = (href: string) => ({
    href,
    onClick: (event: MouseEvent<HTMLElement>) => {
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) {
        return;
      }
      event.preventDefault();
      void navigate({ to: href });
    },
  });
  const sections = ADMIN_SECTIONS.map((section) => ({
    ...section,
    label: t(`admin.sections.${section.id}`),
    current: pathname.startsWith(section.to),
  }));

  return (
    <>
      <Shell>
        <PhoneHeader>
          <IconButton
            icon="chevron-left"
            label={t("admin.back")}
            onClick={() => {
              void navigate({ to: "/" });
            }}
          />
          <Text weight="semibold">{t("admin.title")}</Text>
          <Spacer />
          <AccountMenu compact />
        </PhoneHeader>
        <Sidebar label={t("admin.title")}>
          <div className="flex flex-col gap-1 px-2 pt-1 pb-3">
            <Text weight="semibold">{t("admin.title")}</Text>
            <Text tone="tertiary" size="sm">
              {t("admin.subtitle")}
            </Text>
          </div>
          {sections.map((section) => (
            <NavItem
              key={section.id}
              label={section.label}
              icon={section.icon}
              current={section.current}
              {...link(section.to)}
            />
          ))}
          <SidebarFooter>
            <NavItem label={t("admin.back")} icon="chevron-left" {...link("/")} />
            <AccountMenu />
          </SidebarFooter>
        </Sidebar>
        <Outlet />
      </Shell>
      <TabBar
        phoneOnly
        label={t("admin.title")}
        items={sections.map((section) => ({
          id: section.id,
          label: section.label,
          icon: section.icon,
          current: section.current,
          ...link(section.to),
        }))}
      />
    </>
  );
}
