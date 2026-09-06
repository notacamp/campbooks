# frozen_string_literal: true

module Campbooks
  module Money
    # The month pills over the paired ledger: the month to reconcile first, then
    # every month back to the first statement. A reconciled month is a tab for its
    # statement (two if the same month was reconciled twice); a month with no
    # statement is a muted pill, and a run of them folds into one ("Dec 2025 to
    # Jul 2026 · 8 months without a statement"). Rendered inside the
    # money_statement frame, so the selected pill moves with the ledger.
    class StatementTabs < Campbooks::Base
      MAX_PILLS = 10

      def initialize(page:, **attrs)
        @page  = page
        @attrs = attrs
      end

      def view_template
        nav(class: class_names("flex flex-wrap gap-1.5", @attrs.delete(:class)), role: "tablist", aria_label: t(".title"), **@attrs) do
          pills.first(MAX_PILLS).each { |pill| render_pill(pill) }
          if pills.size > MAX_PILLS
            a(href: helpers.money_statements_path, class: pill_classes(:muted), data: { turbo_frame: "_top" }) { t(".earlier") }
          end
        end
      end

      private

      # [{ kind: :statement, statement: }, { kind: :gap, from:, to:, count:, focus: }] newest first.
      # The month to reconcile always gets its own pill; older gaps fold into runs.
      def pills
        @pills ||= begin
          list = []
          run  = []
          flush = lambda do
            next if run.empty?

            list << { kind: :gap, from: run.last.starts_on, to: run.first.starts_on, count: run.size, focus: false }
            run = []
          end

          @page.months.each do |month|
            if month.reconciled?
              flush.call
              month.statements.sort_by(&:period_end).reverse_each { |stmt| list << { kind: :statement, statement: stmt } }
            elsif month.starts_on == @page.evidence.month_to_reconcile
              list << { kind: :gap, from: month.starts_on, to: month.starts_on, count: 1, focus: true }
            else
              run << month
            end
          end
          flush.call
          list
        end
      end

      def render_pill(pill)
        pill[:kind] == :statement ? statement_pill(pill[:statement]) : gap_pill(pill)
      end

      def statement_pill(stmt)
        selected        = @page.selected_statement&.id == stmt.id
        resolved, total = @page.statement_counts.fetch(stmt.id, [ 0, 0 ])
        has_unresolved  = total.positive? && resolved < total

        a(href: helpers.money_path(statement: stmt.id), class: pill_classes(selected ? :selected : :default), role: "tab",
          aria_selected: selected.to_s, data: { turbo_frame: "money_statement" }) do
          if has_unresolved
            span(class: class_names("h-1.5 w-1.5 rounded-full", selected ? "bg-background" : "bg-ember-gradient shadow-ember-glow"),
                 aria_hidden: "true")
          end
          plain @page.evidence.label_for(stmt)
          span(class: class_names("tabular-nums", selected ? "opacity-70" : "opacity-60")) { plain "#{resolved}/#{total}" }
        end
      end

      def gap_pill(pill)
        label =
          if pill[:count] == 1
            t(pill[:focus] ? ".no_statement_yet" : ".no_statement", month: @page.evidence.month_label(pill[:from]))
          else
            t(".gap", from: l(pill[:from], format: :month_year), to: l(pill[:to], format: :month_year), count: pill[:count])
          end

        a(href: helpers.new_reconciliation_path, class: pill_classes(pill[:focus] ? :focus : :muted),
          data: { turbo_frame: "_top" }, title: t(".add_statement")) { plain label }
      end

      def pill_classes(kind)
        base = "inline-flex h-8 items-center gap-1.5 rounded-full px-3 text-[12.5px] font-medium no-underline transition-colors"
        case kind
        when :selected then class_names(base, "bg-foreground text-background")
        when :focus    then class_names(base, "border border-dashed border-foreground/50 text-foreground hover:bg-muted")
        when :muted    then class_names(base, "border border-dashed border-border text-muted-foreground/80 hover:bg-muted hover:text-foreground")
        else                class_names(base, "border border-border text-muted-foreground hover:bg-muted hover:text-foreground")
        end
      end
    end
  end
end
