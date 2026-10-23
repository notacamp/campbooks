import type { RecordRef } from "~/lib/api";
import { useScoutChat } from "~/lib/scout-chat";
import { IconButton, Visor } from "~/lib/ui";

export interface AskScoutIconProps {
  /** What the chat is about. */
  about: RecordRef;
  /** Its name and tooltip: "Ask Scout". */
  label: string;
}

/**
 * Ask Scout as an icon button with the visor (campbooks-cloud#305), for a
 * dialog's header: the chat opens beside the dialog, which stays as it is;
 * the focus comes back to the button when the chat closes. Nothing where
 * no chat is mounted.
 */
export function AskScoutIcon({ about, label }: AskScoutIconProps) {
  const open = useScoutChat();
  if (open === undefined) {
    return null;
  }
  return (
    <IconButton
      icon={<Visor size="xs" />}
      label={label}
      onClick={(event) => {
        open(about, { from: event.currentTarget });
      }}
    />
  );
}
