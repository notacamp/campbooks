# frozen_string_literal: true

module Campbooks
  module Money
    # Scout's read at the top of the Money page. Assembled from Money::Read data,
    # one clause at a time, with bold amounts and pluralized counts: what the newest
    # statement proves, then the loan. Forbidden words: owe, owed, late, overdue, due.
    class Read < Campbooks::Base
      include Campbooks::Money::Glyphs
      include Campbooks::Money::Ordinals

      def initialize(read:, **attrs)
        @read  = read
        @attrs = attrs
      end

      def view_template
        div(class: class_names("scout-glass rounded-2xl p-4 sm:px-5", @attrs.delete(:class)), **@attrs) do
          div(class: "flex items-center gap-2") do
            render Campbooks::ScoutAvatar.new(size: :xs)
            span(class: "text-[13px] font-bold text-foreground") { "Scout" }
            span(class: "rounded bg-ember-gradient px-1.5 py-0.5 text-[10px] font-bold tracking-wide text-white") { "AI" }
            span(class: "ml-auto text-[11px] text-muted-foreground") do
              if @read.statement_label
                t(".read_statement", label: @read.statement_label)
              else
                t(".no_statements_when")
              end
            end
          end
          p(class: "mt-2.5 text-[14px] leading-relaxed text-foreground/85 sm:text-[14.5px]") do
            raw safe(sentence)
          end
        end
      end

      private

      def sentence
        return no_statements_sentence unless @read.any_statements?

        parts = statement_parts
        parts << loan_sentence if loan_sentence.present?
        parts.join(" ")
      end

      def no_statements_sentence
        t(".no_statements_html", link: helpers.link_to(t(".add_one"), helpers.new_reconciliation_path,
                                                        class: "underline underline-offset-2"))
      end

      def statement_parts # rubocop:disable Metrics/MethodLength
        parts = []

        if @read.statement
          parts << t(".statement_html",
                     label:     @read.statement_label,
                     bank:      @read.bank_name.presence || t(".the_bank"),
                     explained: bold(@read.lines_explained.to_s),
                     total:     bold(@read.lines_total.to_s))
        end

        invoice_clause = invoice_need_clause
        review_clause  = review_need_clause
        if invoice_clause.present? && review_clause.present?
          parts << "#{invoice_clause}, #{review_clause}."
        elsif invoice_clause.present?
          parts << "#{invoice_clause}."
        elsif review_clause.present?
          parts << "#{review_clause.capitalize}."
        end

        if @read.partial_count.positive?
          parts << (@read.partial_count == 1 ? t(".partial_html_one") : t(".partial_html", count: @read.partial_count))
        end

        if @read.missing_count.positive? && @read.statement_label
          parts << (@read.missing_count == 1 ? t(".missing_html_one", label: @read.statement_label) : t(".missing_html", count: @read.missing_count, label: @read.statement_label))
        end

        if @read.lines_explained == @read.lines_total && @read.lines_total.positive? && @read.missing_count.zero?
          parts << t(".all_explained_html")
        end

        parts
      end

      def invoice_need_clause
        return nil if @read.needs_invoice_count.zero?

        @read.needs_invoice_count == 1 ? t(".needs_invoice_html_one") : t(".needs_invoice_html", count: @read.needs_invoice_count)
      end

      def review_need_clause
        return nil if @read.review_count.zero?

        @read.review_count == 1 ? t(".review_html_one") : t(".review_html", count: @read.review_count)
      end

      # ── The loan ─────────────────────────────────────────────────────────────
      def loan_sentence # rubocop:disable Metrics/MethodLength
        @loan_sentence ||=
          case @read.loan_state
          when :suggestion
            s = @read.loan_suggestion
            t(".loan_suggestion_html",
              amount: bold(fmt(s.instalment_cents, s.currency)), lender: s.lender_guess,
              day: day_label(s.day_of_month), count: s.count,
              link: helpers.link_to(t(".loan_suggestion_link"), "#money_needs", class: "underline underline-offset-2"))
          when :changed
            changed = @read.loan.unacknowledged_change
            t(".loan_changed_html", month: l(changed.expected_on, format: :month_name),
                                    from: bold(fmt(changed.previous_amount_cents, @read.loan.currency)),
                                    to: bold(fmt(changed.amount_cents, @read.loan.currency)))
          when :missed
            missed = @read.loan.missed_instalments.first
            t(".loan_missed_html", month: l(missed.expected_on, format: :month_name))
          when :on_schedule, :seen
            seen = @read.loan_last_seen
            key  = @read.loan_state == :on_schedule ? ".loan_on_schedule_html" : ".loan_seen_html"
            t(key, nth: bold(t(".nth_instalment", nth: ordinal(seen.number))),
                   day: day_label(seen.bank_transaction.booked_on.day),
                   remaining: bold(fmt(@read.loan.remaining_cents, @read.loan.currency)))
          when :tracked
            t(".loan_tracked_html")
          end
      end

      def fmt(cents, currency)
        ::Money.new(cents.to_i, currency.presence || "EUR").format(no_cents_if_whole: true)
      end

      def bold(text)
        helpers.tag.b(text)
      end
    end
  end
end
