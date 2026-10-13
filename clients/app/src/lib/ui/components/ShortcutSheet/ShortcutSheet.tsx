import { useTranslation } from "react-i18next";

import { useShortcutDefinitions, type ShortcutGroup } from "~/lib/shortcuts";

import { Overlay } from "../../internal/Overlay";
import { useTranslateKey } from "../../internal/translate-key";
import { Icon } from "../Icon/Icon";
import { IconButton } from "../IconButton/IconButton";
import { Kbd, KbdHint } from "../Kbd/Kbd";

import styles from "./ShortcutSheet.module.css";

const GROUP_ORDER: ShortcutGroup[] = ["navigation", "list", "item", "compose", "general"];

export interface ShortcutSheetProps {
  open: boolean;
  onClose: () => void;
}

/** `?`: every shortcut the registry knows, grouped, with its keys. */
export function ShortcutSheet({ open, onClose }: ShortcutSheetProps) {
  const { t } = useTranslation("ui");
  const translate = useTranslateKey();
  const { definitions } = useShortcutDefinitions();
  return (
    <Overlay open={open} onClose={onClose} label={t("shortcutSheet.title")}>
      <div className={styles["head"]}>
        <Icon name="keyboard" />
        <h2 className={styles["title"]}>{t("shortcutSheet.title")}</h2>
        <Kbd>Esc</Kbd>
        {/* A control in the dialog: it closes it on touch, and lets the
            keyboard reach (and scroll) a long sheet. */}
        <IconButton icon="close" label={t("close")} onClick={onClose} />
      </div>
      <div className={styles["groups"]}>
        {GROUP_ORDER.map((group) => {
          const inGroup = definitions.filter((definition) => definition.group === group);
          if (inGroup.length === 0) {
            return null;
          }
          return (
            <section key={group} className={styles["group"]}>
              <h3 className={styles["groupTitle"]}>{translate(`shortcuts:groups.${group}`)}</h3>
              <dl className={styles["rows"]}>
                {inGroup.map((definition) => (
                  <div key={definition.id} className={styles["row"]}>
                    <dt>{translate(definition.label)}</dt>
                    <dd>
                      <KbdHint shortcut={definition.id} announce />
                    </dd>
                  </div>
                ))}
              </dl>
            </section>
          );
        })}
      </div>
    </Overlay>
  );
}
