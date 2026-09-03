# frozen_string_literal: true

module NowHelper
  # Scout's ledger sentence, assembled from Now::Ledger's honest buckets: the
  # bucket phrases (counts in full ink) joined naturally, then the bold "N need
  # you". States only what the event log proves (docs/messaging.md) — no counts
  # the ledger didn't find, never "handled it all". Returns html_safe.
  def now_ledger_html(ledger)
    lead =
      if ledger.any?
        t("now.index.ledger.lead_html", actions: now_ledger_actions(ledger))
      else
        t("now.index.ledger.empty")
      end

    need = content_tag(:span, t("now.index.ledger.need_you", count: ledger.need_you),
                       class: "font-semibold text-foreground")

    safe_join([ lead, " ", need ])
  end

  # The joined bucket phrases, e.g. "archived <b>24</b> emails, tagged <b>5</b> and
  # filed <b>3</b> documents" — ", " between, " and " before the last.
  def now_ledger_actions(ledger)
    parts = ledger.buckets.map { |bucket| now_ledger_bucket(bucket[:key], bucket[:count], rules: ledger.archived_by_rules?) }
    return "".html_safe if parts.empty?
    return parts.first if parts.size == 1

    safe_join([ safe_join(parts[0..-2], ", "), " #{t('now.index.ledger.and')} ".html_safe, parts.last ])
  end

  # One bucket phrase with its count in full ink. The archive bucket becomes "by
  # your rules" only when Now::Ledger proved every archive was rule-driven.
  def now_ledger_bucket(key, count, rules: false)
    n_html = content_tag(:span, count, class: "font-semibold text-foreground")
    scope = "now.index.ledger.buckets"
    key = :archived_by_rules if key == :archived && rules
    t("#{scope}.#{key}_html", count: count, n_html: n_html)
  end
end
