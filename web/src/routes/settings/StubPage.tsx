/**
 * StubPage — a tasteful "coming soon" placeholder for settings sections
 * that are not yet fully implemented.
 *
 * Used by: Members, AI, Scout's memory, Inbox settings, Integrations,
 * Notifications, Plan, Mailboxes.
 *
 * Each stub page is a thin wrapper around this component that supplies
 * the title and description for the section.
 */
import { type FC, type ElementType } from "react";
import { Visor } from "~/lib/ui";
import { cn } from "~/lib/utils";

interface StubPageProps {
  title: string;
  description: string;
  Icon?: ElementType;
}

export const StubPage: FC<StubPageProps> = ({ title, description }) => (
  <div
    className={cn(
      "flex flex-col items-center justify-center gap-4 py-20 text-center",
      "max-w-md mx-auto",
    )}
    data-testid="settings.page.stub"
  >
    <Visor state="thinking" size={28} label={`${title} coming soon`} />
    <div className="flex flex-col gap-1">
      <h2 className="text-[15px] font-[600] text-t1">{title}</h2>
      <p className="text-[13px] text-t3 leading-relaxed">{description}</p>
    </div>
    <span
      className={cn(
        "inline-flex items-center px-2.5 py-1 rounded-cb-1",
        "bg-surface-1 border border-line text-[11px] text-t4",
        "cb-raised",
      )}
    >
      Coming soon
    </span>
  </div>
);
