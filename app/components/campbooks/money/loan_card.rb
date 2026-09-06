# frozen_string_literal: true

module Campbooks
  module Money
    # "The loan": what the statements prove about a tracked loan. Left: the terms,
    # the big "to go" figure and the instalment tick strip. Right: last seen, what's
    # expected, next, paid so far, the instalment (and when it changed), the actions
    # and the inline edit form (a checkbox-peer disclosure, no JS). Below: the last
    # instalments as the statements show them.
    #
    # Reads only Loan's public helpers so the Lookbook preview can pass a stub.
    #
    # @param loan [Loan]
    class LoanCard < Campbooks::Base
      include Campbooks::Money::Ordinals

      TICK_COLUMNS = 30

      def initialize(loan:, **attrs)
        @loan  = loan
        @attrs = attrs
      end

      def view_template
        section(id: "money_loan", aria_labelledby: "money_loan_h", class: class_names(@attrs.delete(:class)), **@attrs) do
          section_header
          div(class: "grid grid-cols-1 items-start gap-5 pt-1 lg:grid-cols-[1.25fr_1fr] lg:gap-8") do
            left_column
            right_column
          end
          statement_log
        end
      end

      private

      def section_header
        div(class: "mb-2 flex flex-wrap items-baseline gap-2") do
          span(id: "money_loan_h", class: "text-[11px] font-bold uppercase tracking-widest text-muted-foreground") { t(".section_title") }
          span(class: "text-[12px] text-muted-foreground") { subtitle }
        end
      end

      # ── Left ────────────────────────────────────────────────────────────────
      def left_column
        div do
          div(class: "text-[15px] font-semibold tracking-tight text-foreground") { "#{@loan.lender} · #{t('.loan_type')}" }
          div(class: "mt-0.5 text-[12.5px] text-muted-foreground") { terms_line }
          div(class: "mt-3.5 flex flex-wrap items-baseline gap-2") do
            span(class: "text-[26px] font-semibold leading-tight tracking-tight tabular-nums text-foreground") { amount(@loan.remaining_cents) }
            span(class: "text-[13px] text-muted-foreground") { t(".to_go", count: @loan.remaining_count, ends: month_year(@loan.ends_on)) }
          end
          tick_strip
          tick_legend
        end
      end

      def terms_line
        parts = [ amount(@loan.principal_cents) ]
        parts << t(".over_months", count: @loan.term_months, since: month_year(@loan.first_instalment_on))
        parts << t(".instalment_on_day", amount: amount(@loan.instalment_cents), day: day_label(@loan.first_instalment_on.day))
        parts << @loan.rate_note if @loan.rate_note.present?
        parts.join(" · ")
      end

      def tick_strip
        instalments = @loan.instalments.to_a.sort_by(&:number)
        columns     = [ @loan.term_months, TICK_COLUMNS ].min

        div(role: "img", aria_label: tick_aria_label(instalments), class: "mt-3.5 grid max-w-lg gap-0.5",
            style: "grid-template-columns: repeat(#{columns}, minmax(0, 1fr))") do
          instalments.each do |ins|
            span(class: tick_class(ins), title: t(".instalment_n", n: ins.number, total: @loan.term_months))
          end
        end
      end

      # paid = ink (a warning ring where the amount stepped); before your statements
      # = softer ink; awaiting a statement = a dashed outline; missed = warning; to
      # come = subtle.
      def tick_class(ins)
        base = "h-2.5 rounded-sm"
        case ins.status.to_s
        when "paid"       then ins.previous_amount_cents.present? ? "#{base} bg-foreground ring-2 ring-inset ring-warning" : "#{base} bg-foreground"
        when "unverified" then "#{base} bg-foreground/60"
        when "missed"     then "#{base} bg-warning"
        else ins.expected_on < Date.current ? "#{base} border-[1.5px] border-dashed border-foreground/50" : "#{base} bg-subtle"
        end
      end

      def tick_aria_label(instalments)
        paid     = instalments.count { |i| i.paid? || i.unverified? }
        missed   = instalments.count(&:missed?)
        awaiting = instalments.count { |i| i.expected? && i.expected_on < Date.current }
        t(".tick_aria", total: @loan.term_months, paid: paid, expected: awaiting,
                        remaining: @loan.term_months - paid - missed - awaiting)
      end

      def tick_legend
        instalments = @loan.instalments.to_a
        div(class: "mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[11.5px] text-muted-foreground") do
          legend_item("bg-foreground", t(".legend_paid"))
          legend_item("border-[1.5px] border-dashed border-foreground/50", t(".legend_expected")) if instalments.any? { |i| i.expected? && i.expected_on < Date.current }
          legend_item("bg-subtle", t(".legend_to_come"))
          legend_item("bg-warning", t(".legend_missed")) if instalments.any?(&:missed?)
        end
      end

      def legend_item(css, label)
        span(class: "inline-flex items-center gap-1.5") do
          span(class: class_names("inline-block h-2 w-3 rounded-sm", css), aria_hidden: "true")
          plain label
        end
      end

      # ── Right ───────────────────────────────────────────────────────────────
      def right_column
        edit_id = "loan_edit_#{@loan.id}"
        div(class: "flex flex-col gap-2.5") do
          input(type: "checkbox", id: edit_id, class: "peer hidden")
          last_seen_row
          expected_row
          next_row
          paid_so_far_row
          instalment_row
          div(class: "mt-1 flex flex-wrap gap-2") do
            render Campbooks::Button.new(variant: :outline, size: :xs, href: helpers.money_loan_path(@loan),
                                         data: { turbo_frame: "_top" }) { t(".all_instalments", count: @loan.term_months) }
            label(for: edit_id, class: class_names(button_classes(:outline), "peer-checked:hidden")) { t(".edit_terms") }
            stop_tracking_form
          end
          div(class: "hidden peer-checked:block") do
            render Campbooks::Money::LoanForm.new(loan: @loan, toggle_id: edit_id)
          end
        end
      end

      def kv_row(label, &block)
        div(class: "grid grid-cols-[104px_1fr] items-baseline gap-2 text-[13.5px] sm:grid-cols-[116px_1fr]") do
          span(class: "text-[12px] text-muted-foreground") { plain label }
          span(class: "font-medium text-foreground", &block)
        end
      end

      def last_seen_row
        seen = @loan.last_seen
        return unless seen

        txn = seen.bank_transaction
        kv_row(t(".kv_last_seen")) do
          plain l(txn.booked_on, format: :day_month_short)
          statement_link(txn)
          if txn.booked_on <= seen.expected_on + 5.days
            plain " · "
            span(class: "text-[12.5px] font-semibold text-success") { t(".on_time") }
          end
        end
      end

      # The latest instalment whose date has passed with no statement to prove it,
      # and how many earlier ones are in the same position.
      def expected_row
        awaiting = @loan.awaiting_statement.to_a
        ins = awaiting.last
        return unless ins

        kv_row(t(".kv_expected")) do
          plain "#{l(ins.expected_on, format: :day_month_short)} · #{amount(ins.amount_cents)}"
          whitespace
          span(class: "text-[12px] font-normal text-muted-foreground") do
            plain t(".statement_not_in")
            plain " · #{t('.awaiting_others', count: awaiting.size - 1)}" if awaiting.size > 1
          end
        end
      end

      def next_row
        ins = @loan.next_expected
        return unless ins

        kv_row(t(".kv_next")) { plain "#{l(ins.expected_on, format: :day_month_short)} · #{amount(ins.amount_cents)}" }
      end

      def paid_so_far_row
        kv_row(t(".kv_paid_so_far")) do
          plain amount(@loan.paid_cents)
          span(class: "text-[12px] font-normal text-muted-foreground") { plain " #{t('.of')} #{amount(@loan.principal_cents)}" }
        end
      end

      def instalment_row
        kv_row(t(".kv_instalment")) do
          plain amount(@loan.instalment_cents)
          if (changed = @loan.amount_changed_at)
            span(class: "text-[12px] font-normal text-muted-foreground") do
              plain " · #{t('.since_was', since: month_year(changed.expected_on), was: amount(changed.previous_amount_cents))}"
            end
          end
        end
      end

      def stop_tracking_form
        form(action: helpers.money_loan_path(@loan), method: :post, class: "inline-flex",
             data: { turbo_confirm: t(".stop_confirm") }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "_method", value: "delete")
          button(type: "submit", class: class_names(button_classes(:ghost), "text-muted-foreground/70 hover:text-destructive")) { t(".stop_tracking") }
        end
      end

      # ── From the statements ──────────────────────────────────────────────────
      def statement_log
        recent   = @loan.recent_paid(3).to_a
        awaiting = @loan.awaiting_statement.to_a.last
        return if recent.empty? && awaiting.nil?

        div(class: "mt-4 border-t border-border/50 pt-4") do
          span(class: "mb-2 block text-[11px] font-bold uppercase tracking-widest text-muted-foreground") { t(".from_statements") }
          log_row(awaiting.expected_on, amount(awaiting.amount_cents), pending: true) do
            plain t(".expected_statement_not_in", month: l(awaiting.expected_on, format: :month_name))
          end if awaiting
          recent.each do |ins|
            txn = ins.bank_transaction
            log_row(ins.expected_on, "−#{amount(ins.amount_cents)}") do
              plain txn.description
              statement_link(txn)
              if ins.previous_amount_cents.present?
                span(class: "ml-2 rounded border border-warning/40 px-1 text-[10px] font-bold text-warning") { t(".rate_reset") }
                span(class: "ml-1 text-[12px] text-muted-foreground") { t(".rate_changed", from: amount(ins.previous_amount_cents), to: amount(ins.amount_cents)) }
              end
            end
          end
          earlier = @loan.paid_count - recent.size
          if earlier.positive?
            a(href: helpers.money_loan_path(@loan), data: { turbo_frame: "_top" },
              class: "mt-2 inline-block text-[12.5px] text-muted-foreground underline decoration-border/60 underline-offset-2 hover:text-foreground") do
              t(".earlier_instalments", count: earlier)
            end
          end
        end
      end

      def log_row(date, amount_text, pending: false)
        div(class: "grid grid-cols-[72px_1fr_auto] items-baseline gap-3 border-t border-border/40 py-1.5 text-[13px] first:border-t-0 sm:grid-cols-[80px_1fr_auto]") do
          span(class: "text-[12px] text-muted-foreground") { plain l(date, format: :day_month_short) }
          span(class: pending ? "text-muted-foreground" : "text-foreground") { yield }
          span(class: class_names("tabular-nums text-[12.5px] font-medium", pending ? "text-muted-foreground" : "text-foreground")) { plain amount_text }
        end
      end

      def statement_link(txn)
        rec = txn&.reconciliation
        return unless rec

        plain " · "
        a(href: helpers.reconciliation_path(rec, anchor: helpers.dom_id(txn)), data: { turbo_frame: "_top" },
          class: "underline decoration-border underline-offset-2 hover:text-foreground") do
          plain "#{l(txn.booked_on, format: :month_name)} #{t('.statement')}"
        end
      end

      # ── Helpers ──────────────────────────────────────────────────────────────
      def subtitle
        missed = @loan.instalments.to_a.count(&:missed?)
        if missed.positive?
          t(".subtitle_with_missed", count: missed)
        elsif @loan.paid_count >= @loan.term_months
          t(".subtitle_complete")
        else
          t(".subtitle_tracking")
        end
      end

      def month_year(date)
        date ? l(date, format: :month_year) : ""
      end

      def amount(cents)
        return "-" if cents.nil?

        ::Money.new(cents, @loan.currency.presence || "EUR").format(no_cents_if_whole: true)
      end

      def button_classes(variant)
        class_names(Campbooks::Button::BASE_CLASSES, Campbooks::Button::VARIANT_CLASSES[variant], Campbooks::Button::SIZE_CLASSES[:xs], "cursor-pointer")
      end
    end
  end
end
