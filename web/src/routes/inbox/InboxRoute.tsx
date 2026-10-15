/**
 * InboxRoute — binds InboxPage's open conversation to the ?open=<id> search
 * param, so opening a message is a history entry and Back (browser button or
 * phone gesture) closes it.
 */
import { type FC, useCallback } from "react";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { InboxPage } from "./InboxPage";

export const InboxRoute: FC = () => {
  const navigate = useNavigate();
  const { open } = useSearch({ strict: false }) as { open?: string };

  const handleOpenChange = useCallback(
    (id: string | null, { replace }: { replace: boolean }): void => {
      void navigate({ to: "/inbox", search: id == null ? {} : { open: id }, replace });
    },
    [navigate],
  );

  return <InboxPage openId={open ?? null} onOpenChange={handleOpenChange} />;
};
