# frozen_string_literal: true

module Campbooks
  module Money
    # The Statements section header. The month pills and the paired ledger live
    # together inside the `money_statement` turbo-frame that money/_statement_frame
    # renders right after this component (one implementation, shared with the
    # `money#statement` action), so a tab click moves the selection with the
    # ledger. With no statements yet: the empty state.
    class Statements < Campbooks::Base
      def initialize(page:, **attrs)
        @page  = page
        @attrs = attrs
      end

      def view_template
        div(class: @attrs.delete(:class), **@attrs) do
          section_header
          empty_state if @page.evidence.statements.empty?
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
