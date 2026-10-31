import { useId, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, IconButton, Popover } from "@notacamp/ui";

import {
  fromPicked,
  nextWorkingMorning,
  sendTimeLabel,
  sendTimeProblem,
  tomorrowMorning,
} from "./send-time";
import styles from "./SendLater.module.css";

export interface SendLaterProps {
  /** Schedule the draft to leave at this moment. */
  onPick: (at: Date) => void;
  disabled?: boolean | undefined;
}

const two = (n: number) => String(n).padStart(2, "0");
const dateValue = (at: Date) =>
  `${String(at.getFullYear())}-${two(at.getMonth() + 1)}-${two(at.getDate())}`;
const timeValue = (at: Date) => `${two(at.getHours())}:${two(at.getMinutes())}`;

/**
 * Send's chevron: a menu in the top layer (the reader's pinned area would
 * clip an inline one) with tomorrow morning, the next working day and a time
 * of one's own, each showing the moment it resolves to.
 */
export function SendLater({ onPick, disabled }: SendLaterProps) {
  const { t } = useTranslation("ui");
  return (
    <Popover
      label={t("composer.sendLater.title")}
      layer="top"
      placement="bottom-end"
      renderTrigger={(trigger) => (
        <IconButton
          {...trigger}
          label={t("composer.sendLater.title")}
          icon="chevron-down"
          disabled={disabled}
        />
      )}
    >
      {(close) => (
        <SendLaterPanel
          onPick={(at) => {
            close();
            onPick(at);
          }}
        />
      )}
    </Popover>
  );
}

function SendLaterPanel({ onPick }: { onPick: (at: Date) => void }) {
  const { t, i18n } = useTranslation("ui");
  const id = useId();
  // The moments are worked out when the menu opens.
  const [now] = useState(() => new Date());
  const [picking, setPicking] = useState(false);
  const tomorrow = tomorrowMorning(now);
  const [date, setDate] = useState(() => dateValue(tomorrow));
  const [time, setTime] = useState(() => timeValue(tomorrow));
  const [tried, setTried] = useState(false);
  const picked = fromPicked(date, time);
  const problem = sendTimeProblem(picked, new Date());

  if (picking) {
    return (
      <form
        className={styles["form"]}
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          setTried(true);
          if (sendTimeProblem(picked, new Date()) === undefined) {
            onPick(picked);
          }
        }}
      >
        <p className={styles["title"]}>{t("composer.sendLater.pickTitle")}</p>
        <label className={styles["field"]} htmlFor={`${id}-date`}>
          <span>{t("composer.sendLater.date")}</span>
          <input
            id={`${id}-date`}
            type="date"
            value={date}
            min={dateValue(now)}
            data-autofocus
            aria-invalid={tried && problem !== undefined}
            aria-describedby={tried && problem !== undefined ? `${id}-problem` : undefined}
            onChange={(event) => {
              setDate(event.target.value);
            }}
          />
        </label>
        <label className={styles["field"]} htmlFor={`${id}-time`}>
          <span>{t("composer.sendLater.time")}</span>
          <input
            id={`${id}-time`}
            type="time"
            value={time}
            aria-invalid={tried && problem !== undefined}
            onChange={(event) => {
              setTime(event.target.value);
            }}
          />
        </label>
        {tried && problem !== undefined ? (
          <p id={`${id}-problem`} className={styles["problem"]} role="alert">
            {t(`composer.sendLater.problem.${problem}`)}
          </p>
        ) : null}
        <div className={styles["actions"]}>
          <Button
            variant="ghost"
            onClick={() => {
              setPicking(false);
              setTried(false);
            }}
          >
            {t("composer.sendLater.back")}
          </Button>
          <Button variant="primary" type="submit">
            {t("composer.sendLater.schedule")}
          </Button>
        </div>
      </form>
    );
  }

  const row = (label: string, at: Date, key: string) => (
    <Button
      key={key}
      variant="ghost"
      onClick={() => {
        onPick(at);
      }}
    >
      <span className={styles["row"]}>
        <span>{label}</span>
        <span className={styles["when"]}>{sendTimeLabel(at, i18n.language)}</span>
      </span>
    </Button>
  );
  return (
    <div className={styles["menu"]}>
      {row(t("composer.sendLater.tomorrow"), tomorrow, "tomorrow")}
      {row(t("composer.sendLater.workingDay"), nextWorkingMorning(now), "working")}
      <Button
        variant="ghost"
        onClick={() => {
          setPicking(true);
        }}
      >
        {t("composer.sendLater.pick")}
      </Button>
    </div>
  );
}
