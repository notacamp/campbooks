# frozen_string_literal: true

module Campbooks
  module Money
    # The Statements section header and its month tabs. Each tab targets the
    # `money_statement` turbo-frame that money/_statement_frame renders right
    # after this component (one implementation of the frame, shared with the
    # `money#statement` action). With no statements yet: the empty state.
    class Statements < Campbooks::Base
      def initialize(page:, **attrs)
        @page  = page
        @attrs = attrs
      end

      def view_template
        div(class: @attrs.delete(:class), **@attrs) do
          section_header
          if @page.statements.empty?
            empty_state
          else
            tab_bar
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
        nav(class: "flex flex-wrap gap-1.5", role: "tablist", aria_label: t(".title")) do
          @page.statements.each { |stmt| tab_pill(stmt) }
          if @page.more_statements?
            a(href: helpers.money_statements_path, class: pill_classes(false), data: { turbo_frame: "_top" }) { t(".earlier") }
          end
        end
      end

      def tab_pill(stmt)
        selected        = @page.selected_statement&.id == stmt.id
        resolved, total = @page.statement_counts.fetch(stmt.id, [ 0, 0 ])
        has_unresolved  = total.positive? && resolved < total

        a(href: helpers.money_path(statement: stmt.id), class: pill_classes(selected), role: "tab",
          aria_selected: selected.to_s, data: { turbo_frame: "money_statement" }) do
          if has_unresolved
            span(class: class_names("h-1.5 w-1.5 rounded-full", selected ? "bg-background" : "bg-ember-gradient shadow-ember-glow"),
                 aria_hidden: "true")
          end
          plain @page.evidence.label_for(stmt)
          span(class: class_names("tabular-nums", selected ? "opacity-70" : "opacity-60")) { plain "#{resolved}/#{total}" }
        end
      end

      def pill_classes(selected)
        base = "inline-flex h-8 items-center gap-1.5 rounded-full px-3 text-[12.5px] font-medium no-underline transition-colors"
        if selected
          class_names(base, "bg-foreground text-background")
        else
          class_names(base, "border border-border text-muted-foreground hover:bg-muted hover:text-foreground")
        end
      end

      def empty_state
        div(class: "rounded-2xl border border-border bg-card px-6 py-10 text-center") do
          p(class: "text-[15px] font-semibold text-foreground") { t(".empty_title") }
          p(class: "mx-auto mt-1.5 max-w-md text-sm text-muted-foreground") { t(".empty_body") }
          div(class: "mt-4") do
            render(Campbooks::Button.new(variant: :primary, size: :sm, href: helpers.new_reconciliation_path)) { t(".add_statement") }
          end
        end
      end
    end
  end
end
