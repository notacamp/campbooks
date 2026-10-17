import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Button, ChoiceCards, Offer, useUndoToast } from "~/lib/ui";

import { useInboxes, type ConversationSummary } from "../../api/queries";
import {
  dismissSuggestions,
  suggestRules,
  type AheadAction,
  type RuleSuggestions,
} from "../../api/rules";
import type { Translate } from "../../model";
import { needSender } from "../../needs";
import { likenessTitle } from "../../rules-text";

import { SmartRuleConfirm, type SmartRuleDraft } from "./SmartRuleConfirm";

type Step =
  | { step: "idle" }
  | {
      step: "asking";
      conversation: ConversationSummary;
      action: AheadAction;
      /** Who the bill is from, as the row said it (its need is gone by now). */
      payee: string;
      offer: RuleSuggestions;
      chosen: number;
    }
  | { step: "confirming"; draft: SmartRuleDraft };

export interface AheadOfferApi {
  /**
   * After Report and delete or Already paid on `conversation` (as it was
   * before the action): offer the smart rule for mail like it still to come.
   */
  offer: (conversation: ConversationSummary, action: AheadAction) => void;
  /**
   * The action on `conversationId` was undone (any, when not given): its
   * offer, still to come or still asking, goes (a confirmation already open
   * stays, the member chose it).
   */
  cancel: (conversationId?: string) => void;
  /** The offer and its confirmation, rendered once by the screen. */
  element: ReactNode;
  /** An offer is up and holds 1 and Esc: the screen leaves its own unbound. */
  offering: boolean;
}

/**
 * Smart rules offered after an action about mail still to come
 * (campbooks-cloud#169): "Never see emails like this again?" after Report and
 * delete, "Are <payee>'s invoices always paid for you?" after Already paid.
 * Yes (1) opens the smart-rule confirmation, which alone saves; "Just this
 * one" (Esc) turns the offer down (`dismiss_rule_suggestion`).
 */
export function useAheadOffer(workspaceId: string): AheadOfferApi {
  const [state, setState] = useState<Step>({ step: "idle" });
  const asked = useRef(0);
  // The conversation the latest ask is about.
  const askedFor = useRef<string | null>(null);

  const offer = useCallback(
    (conversation: ConversationSummary, action: AheadAction) => {
      const ask = asked.current + 1;
      asked.current = ask;
      askedFor.current = conversation.id;
      const need = conversation.need;
      const payee = need === null || need === undefined ? "" : needSender(need, conversation);
      void suggestRules(workspaceId, conversation.id, action)
        .then((offered) => {
          // A later action asked again meanwhile: that one wins.
          if (asked.current !== ask || offered.suggestions.length === 0) {
            return;
          }
          setState({
            step: "asking",
            conversation,
            action,
            payee,
            offer: offered,
            chosen: offered.default ?? 0,
          });
        })
        // An offer is a nicety: if asking fails, there is simply none.
        .catch(() => null);
    },
    [workspaceId],
  );

  const close = useCallback(() => {
    setState({ step: "idle" });
  }, []);

  const cancel = useCallback((conversationId?: string) => {
    const about = (id: string | null) => conversationId === undefined || id === conversationId;
    if (about(askedFor.current)) {
      asked.current += 1;
    }
    setState((current) =>
      current.step === "asking" && about(current.conversation.id) ? { step: "idle" } : current,
    );
  }, []);

  let element: ReactNode = null;
  if (state.step === "asking") {
    element = (
      <Asking
        // A newer offer starts over (its own default choice).
        key={`${state.conversation.id}:${state.action.kind}`}
        workspaceId={workspaceId}
        state={state}
        onConfirm={(draft) => {
          setState({ step: "confirming", draft });
        }}
        onClose={close}
      />
    );
  } else if (state.step === "confirming") {
    element = <SmartRuleConfirm workspaceId={workspaceId} draft={state.draft} onClose={close} />;
  }
  return { offer, cancel, element, offering: state.step !== "idle" };
}

function Asking({
  workspaceId,
  state,
  onConfirm,
  onClose,
}: {
  workspaceId: string;
  state: Extract<Step, { step: "asking" }>;
  onConfirm: (draft: SmartRuleDraft) => void;
  onClose: () => void;
}) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const undo = useUndoToast();
  const inboxes = useInboxes(workspaceId);
  const ref = useRef<HTMLDivElement>(null);
  const [chosen, setChosen] = useState(state.chosen);
  const { conversation, action, offer, payee } = state;
  const suggestion = offer.suggestions[chosen] ?? offer.suggestions[0];
  const inbox = inboxes.data?.find((item) => item.id === conversation.inbox_id);

  // Brought to the offer's first action, so Enter and Esc are its own.
  useEffect(() => {
    ref.current?.querySelector<HTMLButtonElement>("button:not([disabled])")?.focus();
  }, []);

  const value = (field: "sender_address" | "sender_domain") => {
    const found = suggestion?.conditions.find((condition) => condition.field === field);
    return found !== undefined && "value" in found && typeof found.value === "string"
      ? found.value
      : "";
  };
  const pretending = suggestion?.likeness === "impersonation";
  const report = action.kind === "report";
  const title = report ? t("ahead.reportTitle") : t("ahead.paidTitle", { payee });
  const detail = report
    ? pretending
      ? t("ahead.reportPretending", { domain: value("sender_domain") })
      : t("ahead.reportSender", {
          sender: value("sender_address") || value("sender_domain"),
        })
    : t("ahead.paidDetail", { payee });

  const accept = () => {
    if (suggestion === undefined) {
      return;
    }
    onConfirm({
      inboxId: conversation.inbox_id,
      conditions: suggestion.conditions,
      actions: suggestion.actions,
      title: report ? t("smartRule.titleReport") : t("smartRule.titlePaid", { payee }),
      origin: "action",
      fromConversation: conversation.id,
      sharedInbox: offer.shared && inbox !== undefined ? inbox.address : undefined,
    });
  };
  const dismiss = () => {
    dismissSuggestions(
      workspaceId,
      conversation.id,
      offer.suggestions.map((each) => each.key),
    ).catch(() => {
      undo.notify(t("rules.offer.dismissFailed"));
    });
    onClose();
  };
  useShortcut("item.choice1", accept, { enabled: suggestion !== undefined });
  useShortcut("overlay.close", dismiss);

  return (
    <div ref={ref} className="contents">
      <Offer
        title={title}
        detail={detail}
        actions={
          <>
            <Button variant="secondary" shortcut="item.choice1" onClick={accept}>
              {t("ahead.accept")}
            </Button>
            <Button variant="secondary" shortcut="overlay.close" onClick={dismiss}>
              {t("rules.offer.justThis")}
            </Button>
          </>
        }
      >
        {offer.suggestions.length > 1 ? (
          <ChoiceCards
            label={t("rules.offer.which")}
            hideLabel
            value={String(chosen)}
            onChange={(next) => {
              setChosen(Number(next));
            }}
            options={offer.suggestions.map((each, index) => ({
              value: String(index),
              title: likenessTitle(t, each),
            }))}
          />
        ) : null}
      </Offer>
    </div>
  );
}
