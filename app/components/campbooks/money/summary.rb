# frozen_string_literal: true

module Campbooks
  module Money
    # Scout's read at the top of the Money page — the Ember-glass note from the mock.
    # It states only what the obligations prove (owed / owe / late / renewals) and
    # NEVER claims solvency: the strongest close it will make is "Nothing else is due
    # this month." Amounts are bold; the sentence is assembled from Money::Summary.
    class Summary < Campbooks::Base
      def initialize(summary:, **attrs)
        @summary = summary
        @attrs = attrs
      end

      def view_template
        div(class: class_names("scout-glass rounded-2xl p-4 sm:px-5", @attrs.delete(:class)), **@attrs) do
          div(class: "flex items-center gap-2") do
            render Campbooks::ScoutAvatar.new(size: :xs)
            span(class: "text-[13px] font-bold text-foreground") { "Scout" }
            span(class: "rounded bg-ember-gradient px-1.5 py-0.5 text-[10px] font-bold tracking-wide text-white") { "AI" }
            span(class: "ml-auto text-[11px] text-muted-foreground") { t(".today") }
          end
          p(class: "mt-2.5 text-[14px] leading-relaxed text-foreground/85 sm:text-[14.5px]") do
            raw safe(sentence.to_s)
          end
        end
      end

      private

      def sentence
        parts = []
        parts << t(".owed_html", amount: bold(fmt(@summary.owed_to_you_total))) if @summary.owed_to_you?

        # When matters_most is a late receivable, use the richer matters clause and
        # skip the separate late_receivable sentence. For late payables, use matters
        # clause in place of the old late_payable_clause.
        mp = @summary.matters_most
        if mp&.late? && mp&.receivable?
          parts << matters_clause(mp)
        else
          parts << late_receivable_clause if @summary.late_receivable
        end

        parts << owe_clause if @summary.you_owe?

        if mp&.late? && mp&.payable?
          parts << matters_clause(mp)
        else
          parts << late_payable_clause if @summary.late_payable
        end

        parts << renewal_clause if @summary.renewals.any?
        parts << t(".nothing_else") if @summary.nothing_else_this_month? && parts.any?
        parts << t(".clear") if parts.empty?
        helpers.safe_join(parts, " ")
      end

      def owe_clause
        t(".owe_html", amount: bold(fmt(@summary.you_owe_total)), count: @summary.owe_count)
      end

      def late_receivable_clause
        o = @summary.late_receivable
        t(".late_receivable_html", name: o.counterpart, overdue: overdue_phrase(o))
      end

      def late_payable_clause
        o = @summary.late_payable
        t(".late_payable_html", name: o.counterpart, overdue: overdue_phrase(o))
      end

      # Richer clause for the highest-priority late obligation (any direction). With
      # nothing learned about it yet there is no "why" to append, so the sentence
      # ends on the overdue phrase instead of a dangling dash.
      def matters_clause(o)
        why_text = Array(o.why).join(", ")
        if why_text.blank?
          t(".matters_plain_html", name: o.counterpart, amount: bold(fmt(o.amount)), overdue: overdue_phrase(o))
        else
          t(".matters_html", name: o.counterpart, amount: bold(fmt(o.amount)), overdue: overdue_phrase(o), why: why_text)
        end
      end

      def renewal_clause
        o = @summary.renewals.min_by(&:due_on)
        t(".renewal_html", amount: bold(fmt(o.amount)), date: l(o.due_on, format: :date))
      end

      def overdue_phrase(obligation)
        t("money.status.late", count: obligation.days_late(Date.current))
      end

      # Whole amounts drop the cents in the sentence ("€3,420"), matching the mock;
      # the ledger table keeps the full precision.
      def fmt(money)
        return "" if money.nil?

        money.format(no_cents_if_whole: true)
      end

      def bold(text)
        helpers.tag.b(text)
      end
    end
  end
end
