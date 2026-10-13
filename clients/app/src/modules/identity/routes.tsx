import {
  createRoute,
  useLocation,
  useNavigate,
  useParams,
  useSearch,
  type AnyRoute,
} from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { NewAccountScreen } from "./components/NewAccount/NewAccount";
import { SignInScreen } from "./components/SignIn/SignInScreen";
import { WaysInPage } from "./components/WaysIn/WaysInPage";
import { WAY_IN_SECTIONS } from "./components/WaysIn/sections";
import { linkResult } from "./model";

/** The sign-in screens' query: what the issuer and the app send them. */
interface SignInSearch {
  request?: string;
  error?: string;
  return_to?: string;
}

const text = (value: unknown) => (typeof value === "string" && value !== "" ? value : undefined);

function validateSignInSearch(search: Record<string, unknown>): SignInSearch {
  const request = text(search["request"]);
  const error = text(search["error"]);
  const returnTo = text(search["return_to"]);
  return {
    ...(request === undefined ? {} : { request }),
    ...(error === undefined ? {} : { error }),
    ...(returnTo === undefined ? {} : { return_to: returnTo }),
  };
}

/** What the front door comes back to Ways to sign in with. */
interface WaysInSearch {
  linked?: string;
  connected?: string;
  link_error?: string;
}

function validateWaysInSearch(search: Record<string, unknown>): WaysInSearch {
  const linked = text(search["linked"]);
  const connected = text(search["connected"]);
  const linkError = text(search["link_error"]);
  return {
    ...(linked === undefined ? {} : { linked }),
    ...(connected === undefined ? {} : { connected }),
    ...(linkError === undefined ? {} : { link_error: linkError }),
  };
}

/**
 * Settings → Ways to sign in. The result the front door came back with is
 * read once, then cleared from the address bar (a reload shows the list).
 */
function WaysInRoute() {
  const { workspaceId } = useParams({ strict: false });
  const search: WaysInSearch = useSearch({ strict: false });
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const [result] = useState(() => linkResult(search));
  const hasResult = Object.keys(search).length > 0;
  useEffect(() => {
    if (hasResult) {
      void navigate({ to: pathname, search: {}, replace: true });
    }
  }, [hasResult, navigate, pathname]);
  return workspaceId === undefined ? null : (
    <WaysInPage
      workspaceId={workspaceId}
      result={result}
      returnTo={pathname}
      sections={WAY_IN_SECTIONS}
    />
  );
}

export interface IdentityRouteParents<TPublic extends AnyRoute, TWorkspace extends AnyRoute> {
  /** Pages anyone can open (the sign-in screens). */
  public: TPublic;
  /** `/w/$workspaceId`: where settings live. */
  workspace: TWorkspace;
}

/**
 * `/sign-in` and `/sign-in/new`, public: they are how a browser gets a
 * session; and Settings → Ways to sign in, inside a workspace.
 */
export function buildIdentityRoutes<TPublic extends AnyRoute, TWorkspace extends AnyRoute>(
  parents: IdentityRouteParents<TPublic, TWorkspace>,
) {
  const parent = parents.public;
  const signIn = createRoute({
    getParentRoute: () => parent,
    path: "/sign-in",
    validateSearch: validateSignInSearch,
    component: function SignInRoute() {
      const search: SignInSearch = useSearch({ strict: false });
      return (
        <SignInScreen request={search.request} error={search.error} returnTo={search.return_to} />
      );
    },
  });
  const newAccount = createRoute({
    getParentRoute: () => parent,
    path: "/sign-in/new",
    validateSearch: validateSignInSearch,
    component: function NewAccountRoute() {
      const search: SignInSearch = useSearch({ strict: false });
      return <NewAccountScreen request={search.request} />;
    },
  });
  const waysIn = createRoute({
    getParentRoute: () => parents.workspace,
    path: "/settings/sign-in",
    validateSearch: validateWaysInSearch,
    component: WaysInRoute,
  });
  return { public: [signIn, newAccount] as const, workspace: [waysIn] as const };
}
