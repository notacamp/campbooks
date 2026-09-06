# frozen_string_literal: true

module Campbooks
  module Money
    # "The loan" section from the prototype. Renders the tracked loan's full card:
    # - Lender heading + terms line
    # - Big "remaining" figure
    # - Instalment tick strip (60 ticks, colour-coded by status)
    # - Key/value rows: last seen, expected, next, paid so far, instalment change
    # - Actions: "All N instalments", "Edit the terms", "Stop tracking"
    # - Full-width footer "From the statements" (last 3 paid + next expected)
    #
    # @param loan     [Loan]
    # @param evidence [Money::Evidence, nil]  — optional PR A coverage object
    class LoanCard < Campbooks::Base
      def initialize(loan:, evidence: nil)
        @loan     = loan
        @evidence = evidence
      end

      def view_template
        section(id: "money_loan", aria_labelledby: "money_loan_h",
                class: "mt-9") do
          # Section header
          div(class: "flex items-baseline gap-2 mb-2 flex-wrap") do
            span(id: "money_loan_h",
                 class: "text-[11.5px] font-bold tracking-widest uppercase text-muted-foreground") { plain t(".section_title") }
            span(class: "text-[11.5px] text-muted-foreground") { plain subtitle }
          end

          # Two-column grid
          div(class: "grid grid-cols-1 lg:grid-cols-[1.25fr_1fr] gap-4 lg:gap-8 items-start pt-1.5") do
            left_col
            right_col
          end

          # Full-width footer: statement log
          div(class: "mt-4 pt-4 border-t border-border/50") do
            statement_log
          end
        end
      end

      private

      # ── Left column ─────────────────────────────────────────────────────────────

      def left_col
        div do
          # Lender + type title
          div(class: "text-[15px] font-semibold tracking-tight") do
            plain "#{@loan.lender} · #{t(".loan_type")}"
          end
          div(class: "text-[12.5px] text-muted-foreground mt-0.5") { plain terms_line }

          # Big remaining figure
          div(class: "mt-3.5 flex items-baseline gap-2 flex-wrap") do
            span(class: "text-[26px] font-semibold tracking-tight tabular-nums leading-tight") do
              plain format_amount(@loan.remaining_cents)
            end
            span(class: "text-[13px] text-muted-foreground") do
              plain t(".to_go", count: @loan.remaining_count, ends: ends_label)
            end
          end

          # Tick strip
          tick_strip
          tick_legend
        end
      end

      def terms_line
        parts = []
        parts << format_amount(@loan.principal_cents) if @loan.principal_cents.positive?
        if @loan.term_months.positive?
          parts << t(".over_months", count: @loan.term_months, since: since_label)
        end
        if @loan.instalment_cents.positive? && @loan.first_instalment_on.present?
          parts << t(".instalment_on_day",
                     amount: format_amount(@loan.instalment_cents),
                     day:    @loan.first_instalment_on.day)
        end
        parts << @loan.rate_note if @loan.rate_note.present?
        parts.join(" · ")
      end

      def since_label
        return "" unless @loan.first_instalment_on

        I18n.l(@loan.first_instalment_on, format: :month_year)
      end

      def ends_label
        return "" unless @loan.ends_on

        I18n.l(@loan.ends_on, format: :month_year)
      end

      def tick_strip
        instalments = @loan.instalments.ordered.to_a
        total = @loan.term_months

        div(role: "img",
            aria_label: tick_aria_label(instalments, total),
            class: "mt-3.5 grid gap-0.5 max-w-lg",
            style: "grid-template-columns: repeat(#{[ total, 30 ].min}, minmax(0, 1fr))") do
          instalments.first(total).each do |ins|
            span(class: tick_class(ins), title: t(".instalment_n", n: ins.number, total: total))
          end
        end
      end

      def tick_class(ins)
        base = "h-2.5 rounded-sm"
        case ins.status.to_sym
        when :paid
          if ins.previous_amount_cents.present?
            "#{base} bg-foreground ring-2 ring-warning ring-inset"
          else
            "#{base} bg-foreground"
          end
        when :unverified
          "#{base} bg-foreground"
        when :missed
          "#{base} bg-warning"
        when :expected
          if ins == @loan.next_expected
            "#{base} border-[1.5px] border-foreground"
          else
            "#{base} bg-subtle"
          end
        else
          "#{base} bg-subtle"
        end
      end

      def tick_aria_label(instalments, total)
        paid    = instalments.count { |i| i.paid? || i.unverified? }
        missed  = instalments.count(&:missed?)
        expected = instalments.count(&:expected?)
        t(".tick_aria", total: total, paid: paid, expected: expected, remaining: total - paid - missed)
      end

      def tick_legend
        div(class: "mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[11.5px] text-muted-foreground") do
          legend_item("bg-foreground rounded-sm", t(".legend_paid"))
          legend_item("border-[1.5px] border-foreground rounded-sm", t(".legend_expected"))
          legend_item("bg-subtle rounded-sm", t(".legend_to_come"))
          if @loan.instalments.any?(&:missed?)
            legend_item("bg-warning rounded-sm", t(".legend_missed"))
          end
        end
      end

      def legend_item(css, label)
        span(class: "inline-flex items-center gap-1.5") do
          span(class: "inline-block w-3 h-2 #{css}", aria_hidden: "true")
          plain label
        end
      end

      # ── Right column (KV rows) ───────────────────────────────────────────────────

      def right_col
        div(class: "flex flex-col gap-2.5") do
          last_seen_row
          expected_row
          next_row
          paid_so_far_row
          instalment_row

          div(class: "mt-1 flex flex-wrap gap-2") do
            link_to t(".all_instalments", count: @loan.term_months),
                    helpers.money_loan_path(@loan),
                    class: "inline-flex items-center px-2.5 py-1 rounded-lg border border-border text-[12px] font-medium text-muted-foreground hover:bg-muted/40 transition-colors"

            # "Edit the terms" — discloses the form panel
            button(type: "button",
                   class: "inline-flex items-center px-2.5 py-1 rounded-lg border border-border text-[12px] font-medium text-muted-foreground hover:bg-muted/40 transition-colors",
                   data: { action: "click->loan-form#openEdit" }) do
              plain t(".edit_terms")
            end

            helpers.button_to(
              t(".stop_tracking"),
              helpers.money_loan_path(@loan),
              method: :delete,
              data:   { turbo_confirm: t(".stop_confirm") },
              class:  "inline-flex items-center px-2.5 py-1 rounded-lg text-[12px] font-medium text-muted-foreground/70 hover:text-danger hover:bg-danger/5 transition-colors",
              form:   { data: { turbo: true } }
            )
          end

          # Edit form (hidden, toggled by Stimulus)
          div(class: "hidden mt-2", data: { loan_form_target: "editPanel" }) do
            render(Campbooks::Money::LoanForm.new(loan: @loan))
          end
        end
      end

      def kv_row(label, &block)
        div(class: "grid grid-cols-[116px_1fr] gap-2 items-baseline text-[13.5px]") do
          span(class: "text-[12px] text-muted-foreground") { plain label }
          span(class: "font-medium", &block)
        end
      end

      def last_seen_row
        ins = @loan.last_seen
        return unless ins

        txn = ins.bank_transaction
        kv_row(t(".kv_last_seen")) do
          date_s = I18n.l(txn.booked_on, format: :day_month_short) if txn
          on_time = txn && ins.expected_on.present? && txn.booked_on <= ins.expected_on + 5.days
          parts = [ date_s ]

          if txn&.reconciliation
            parts << helpers.link_to(
              I18n.l(txn.booked_on, format: :month_year) + " " + t(".statement"),
              helpers.reconciliation_path(txn.reconciliation, anchor: helpers.dom_id(txn)),
              class: "text-foreground underline decoration-border underline-offset-2"
            )
          end

          plain parts.compact.join(" · ")
          if on_time
            whitespace
            span(class: "text-success font-semibold text-[12.5px]") { plain t(".on_time") }
          end
        end
      end

      def expected_row
        ins = @loan.instalments.where(status: :expected).order(:expected_on).first
        return unless ins && ins.expected_on <= Date.current + 31.days

        kv_row(t(".kv_expected")) do
          plain "#{I18n.l(ins.expected_on, format: :day_month_short)} · #{format_amount(ins.amount_cents)}"
          whitespace
          span(class: "text-[12px] text-muted-foreground font-normal") { plain t(".statement_not_in") }
        end
      end

      def next_row
        ins = @loan.next_expected
        return unless ins

        kv_row(t(".kv_next")) do
          plain "#{I18n.l(ins.expected_on, format: :day_month_short)} · #{format_amount(ins.amount_cents)}"
        end
      end

      def paid_so_far_row
        kv_row(t(".kv_paid_so_far")) do
          plain format_amount(@loan.paid_cents)
          span(class: "text-[12px] text-muted-foreground font-normal") do
            plain " #{t(".of")} #{format_amount(@loan.principal_cents)}"
          end
        end
      end

      def instalment_row
        kv_row(t(".kv_instalment")) do
          plain format_amount(@loan.instalment_cents)
          if (changed = @loan.amount_changed_at)
            prev = format_amount(changed.previous_amount_cents)
            span(class: "text-[12px] text-muted-foreground font-normal") do
              plain " · #{t(".since_was", since: I18n.l(changed.expected_on, format: :month_year), was: prev)}"
            end
          end
        end
      end

      # ── Statement log ─────────────────────────────────────────────────────────────

      def statement_log
        span(class: "text-[11px] font-bold tracking-widest uppercase text-muted-foreground block mb-2") do
          plain t(".from_statements")
        end

        # Next expected (faint)
        if (nxt = @loan.next_expected)
          div(class: "grid grid-cols-[80px_1fr_auto] gap-3 items-baseline py-1.5 text-[13px]") do
            span(class: "text-muted-foreground text-[12px]") { plain I18n.l(nxt.expected_on, format: :day_month_short) }
            span(class: "text-muted-foreground") do
              plain t(".expected_statement_not_in", month: I18n.l(nxt.expected_on, format: :month_year))
            end
            span(class: "text-muted-foreground tabular-nums text-[12px] font-medium") do
              plain format_amount(nxt.amount_cents)
            end
          end
        end

        # Last 3 paid
        recent = @loan.instalments
                      .where(status: %i[paid unverified])
                      .where.not(bank_transaction_id: nil)
                      .includes(bank_transaction: :reconciliation)
                      .order(expected_on: :desc)
                      .limit(3)

        recent.each do |ins|
          txn = ins.bank_transaction
          div(class: "grid grid-cols-[80px_1fr_auto] gap-3 items-baseline py-1.5 border-t border-border/40 text-[13px]") do
            span(class: "text-muted-foreground text-[12px]") { plain I18n.l(ins.expected_on, format: :day_month_short) }
            span do
              plain txn.description
              plain " · "
              rec = txn.reconciliation
              if rec
                plain helpers.link_to(
                  I18n.l(ins.expected_on, format: :month_year) + " " + t(".statement"),
                  helpers.reconciliation_path(rec, anchor: helpers.dom_id(txn)),
                  class: "text-muted-foreground underline decoration-border/60 underline-offset-2"
                )
              end
              if ins.previous_amount_cents.present?
                span(class: "ml-2 text-[11px] font-bold text-warning border border-warning/40 rounded px-1") do
                  plain t(".rate_reset")
                end
                span(class: "text-[12px] text-muted-foreground ml-1") do
                  plain t(".rate_changed", from: format_amount(ins.previous_amount_cents),
                                           to:   format_amount(ins.amount_cents))
                end
              end
            end
            span(class: "text-foreground tabular-nums font-medium") do
              plain "-#{format_amount(ins.amount_cents)}"
            end
          end
        end

        # "N earlier instalments" link
        earlier = @loan.paid_count - recent.size
        if earlier > 0
          link_to t(".earlier_instalments", count: earlier),
                  helpers.money_loan_path(@loan),
                  class: "inline-block mt-2 text-[12.5px] text-muted-foreground underline decoration-border/60 underline-offset-2 hover:text-foreground"
        end
      end

      # ── Helpers ───────────────────────────────────────────────────────────────────

      def subtitle
        all_paid = @loan.instalments.where(status: %i[paid unverified]).count
        missed   = @loan.instalments.where(status: :missed).count

        if missed > 0
          t(".subtitle_with_missed", count: missed)
        elsif all_paid == @loan.term_months
          t(".subtitle_complete")
        else
          t(".subtitle_tracking")
        end
      end

      def format_amount(cents)
        return "-" if cents.nil?

        symbol = { "EUR" => "€", "USD" => "$", "GBP" => "£" }.fetch(@loan.currency.upcase, @loan.currency)
        whole  = cents % 100 == 0
        whole ? "#{symbol}#{cents / 100}" : "#{symbol}#{sprintf("%.2f", cents / 100.0)}"
      end
    end
  end
end
