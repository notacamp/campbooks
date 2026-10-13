import { createRoute, useSearch, type AnyRoute } from "@tanstack/react-router";

import { NewAccountScreen } from "./components/NewAccount/NewAccount";
import { SignInScreen } from "./components/SignIn/SignInScreen";

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

/** `/sign-in` and `/sign-in/new`, public: they are how a browser gets a session. */
export function buildIdentityRoutes<TPublic extends AnyRoute>(parent: TPublic) {
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
  return [signIn, newAccount] as const;
}
