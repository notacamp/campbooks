import { useTranslation } from "react-i18next";

import { IconButton, Popover, PopoverSection } from "~/lib/ui";

import { iconOf, PAGE_ICONS, type PageIcon } from "../types";

export interface IconPickerProps {
  /** The page's icon as stored (none yet is the document icon). */
  value: string | null | undefined;
  onChange: (icon: PageIcon) => void;
}

/** A page's icon: just the icon as a button, and the choice of icons in a panel. */
export function IconPicker({ value, onChange }: IconPickerProps) {
  const { t } = useTranslation("pages");
  const current = iconOf(value);
  return (
    <Popover
      label={t("screen.icon")}
      renderTrigger={(trigger) => (
        <IconButton
          {...trigger}
          icon={current}
          size="lg"
          label={t("screen.iconCurrent", { name: t(`icon.${current}`) })}
        />
      )}
    >
      {(close) => (
        <PopoverSection title={t("screen.icon")}>
          <div className="grid grid-cols-4 gap-1">
            {PAGE_ICONS.map((name) => (
              <IconButton
                key={name}
                icon={name}
                label={t(`icon.${name}`)}
                aria-pressed={name === current}
                onClick={() => {
                  close();
                  onChange(name);
                }}
              />
            ))}
          </div>
        </PopoverSection>
      )}
    </Popover>
  );
}
