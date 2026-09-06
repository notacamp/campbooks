# frozen_string_literal: true

module Campbooks
  module Money
    # The Statements section: pill tabs for the recent reconciliations, each
    # loading the grouped ledger into the money_statement turbo frame.
    class Statements < Campbooks::Base
      def initialize(page:, **attrs)
        @page  = page
        @attrs = attrs
      end

      def view_template
        div(class: @attrs.delete(:class), **@attrs) do
          section_header
          if @page.statements.empty?
            render Campbooks::EmptyState.new(
              title: t(".empty_title"),
              body: t(".empty_body"),
              action_label: t(".add_statement"),
              action_href: helpers.new_reconciliation_path
            )
          else
            tab_bar
            statement_frame
          end
        end
      end

      private

      def section_header
        div(class: "mb-3 flex flex-wrap items-baseline gap-2") do
          span(class: "text-[11px] font-bold uppercase tracking-widest text-muted-foreground") do
            plain t(".title")
            bank = @page.read.bank_name.presence
            plain " · #{bank}" if bank
          end
          span(class: "ml-auto text-[12px] text-muted-foreground") { t(".note") }
        end
      end

      def tab_bar
        nav(class: "flex flex-wrap gap-1.5", role: "tablist") do
          @page.statements.each do |stmt|
            tab_pill(stmt)
          end
          if @page.more_statements?
            a(href: helpers.money_statements_path,
              class: "inline-flex h-8 items-center rounded-full border border-border px-3 text-[12.5px] font-medium text-muted-foreground hover:bg-muted transition-colors no-underline",
              data: { turbo_frame: "_top" }) do
              plain t(".earlier")
            end
          end
        end
      end

      def tab_pill(stmt)
        selected = @page.selected_statement&.id == stmt.id
        evidence = @page.evidence
        label    = evidence.label_for(stmt)
        resolved = stmt.instance_variable_get(:@resolved_count) || stmt.bank_transactions.where(status: BankTransaction::RESOLVED_STATUSES).count
        total    = stmt.instance_variable_get(:@total_transactions) || stmt.bank_transactions.count

        has_unresolved = total.positive? && resolved < total

        css = if selected
          "inline-flex h-8 items-center gap-1.5 rounded-full bg-foreground px-3 text-[12.5px] font-medium text-background no-underline"
        else
          "inline-flex h-8 items-center gap-1.5 rounded-full border border-border px-3 text-[12.5px] font-medium text-muted-foreground hover:bg-muted hover:text-foreground transition-colors no-underline"
        end

        a(href: helpers.money_path(statement: stmt.id),
          class: css,
          role: "tab",
          aria: { selected: selected.to_s },
          data: { turbo_frame: "money_statement" }) do
          if has_unresolved
            span(class: "h-1.5 w-1.5 rounded-full bg-ember-gradient shadow-ember-glow")
          end
          plain label
          span(class: selected ? "opacity-60" : "opacity-40") { plain " #{resolved}/#{total}" }
        end
      end

      def statement_frame
        helpers.turbo_frame_tag("money_statement", class: "block mt-4") do
          frame_content
        end
      end

      def frame_content
        return unless @page.selected_statement

        stmt = @page.selected_statement
        evidence = @page.evidence
        label = evidence.label_for(stmt)

        render Campbooks::Accounting::GroupedLedger.new(
          groups:         @page.selected_groups,
          reconciliation: stmt,
          company_nif:    @page.company_nif
        )

        div(class: "mt-3 text-right") do
          a(href: helpers.reconciliation_path(stmt),
            class: "text-[12.5px] text-muted-foreground underline underline-offset-2 hover:text-foreground",
            data: { turbo_frame: "_top" }) do
            plain t(".all_lines", label: label, count: stmt.total_transactions)
          end
        end
      end
    end
  end
end
