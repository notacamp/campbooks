# frozen_string_literal: true

module Campbooks
  module Money
    # "Needs you" section: flat rows, Ember dot at left, actions at right.
    # Hidden when there are no items.
    class NeedsYou < Campbooks::Base
      def initialize(items:, overflow:, statement:, **attrs)
        @items     = items
        @overflow  = overflow
        @statement = statement
        @attrs     = attrs
      end

      def view_template
        return if @items.empty?

        div(class: @attrs.delete(:class), **@attrs) do
          section_header
          div(class: "divide-y divide-border/50") do
            @items.each { |item| row(item) }
          end
          overflow_note if @overflow.positive?
        end
      end

      private

      def section_header
        div(class: "mb-1 flex flex-wrap items-baseline gap-2") do
          span(class: "text-[11px] font-bold uppercase tracking-widest text-muted-foreground") do
            plain "#{t('.title')} "
            span(class: "font-normal normal-case tracking-normal text-muted-foreground/70") { plain "· #{@items.size}" }
          end
          if @statement
            span(class: "ml-auto text-[12px] text-muted-foreground") { t(".note") }
          end
        end
      end

      def row(item)
        root_attrs = row_attrs(item)
        div(**root_attrs) do
          # Ember dot
          span(class: "mt-[3px] h-2 w-2 shrink-0 rounded-full bg-ember-gradient shadow-ember-glow")
          div(class: "min-w-0 flex-1") do
            div(class: "text-[14px] font-semibold text-foreground leading-snug") { plain item.title }
            if item.meta.present?
              div(class: "mt-0.5 flex flex-wrap items-center gap-x-1.5 gap-y-0.5 text-[12.5px] text-muted-foreground") do
                item.meta.each_with_index do |part, i|
                  span(class: "opacity-40 mx-0.5") { plain "·" } if i.positive?
                  plain part.to_s
                end
              end
            end
          end
          div(class: "flex shrink-0 flex-wrap items-center gap-2") do
            render_actions(item)
          end
        end
      end

      def row_attrs(item)
        base = {
          class: "-mx-4 flex items-start gap-3 px-4 py-3.5 rounded-xl transition-colors hover:bg-muted/50 sm:-mx-6 sm:px-6"
        }

        if item.kind == :no_invoice && item.transaction
          txn = item.transaction
          stmt = @statement
          base[:id]   = helpers.dom_id(txn)
          base[:data] = {
            controller:                        "transaction-resolve",
            transaction_resolve_url_value:      helpers.resolve_panel_reconciliation_bank_transaction_path(stmt, txn, surface: "money"),
            transaction_resolve_frame_id_value: helpers.dom_id(txn, :resolve_frame)
          }
        end

        base
      end

      def render_actions(item) # rubocop:disable Metrics/MethodLength
        case item.kind
        when :no_invoice
          if item.transaction && @statement
            helpers.button_tag(t(".resolve"),
                               class: "inline-flex h-[30px] items-center rounded-lg border border-border bg-background px-3 text-[12.5px] font-medium text-foreground hover:bg-secondary transition-colors",
                               data: { action: "click->transaction-resolve#toggle" })
          end
        when :review
          if item.transaction && @statement
            link_change = helpers.link_to(t(".change"),
                                          helpers.reconciliation_path(@statement, anchor: helpers.dom_id(item.transaction)),
                                          class: "inline-flex h-[30px] items-center rounded-lg border border-border px-3 text-[12.5px] font-medium text-muted-foreground hover:text-foreground no-underline transition-colors",
                                          data: { turbo_frame: "_top" })
            raw safe(link_change)
            if item.match
              helpers.button_to(t(".confirm"),
                                helpers.confirm_line_money_path(item.transaction.id),
                                params: { match_id: item.match.id },
                                class: "inline-flex h-[30px] items-center rounded-lg bg-foreground px-3 text-[12.5px] font-medium text-background hover:opacity-80 transition-opacity cursor-pointer",
                                method: :post)
            end
          end
        when :partial
          if @statement
            helpers.link_to(t(".open_statement"),
                            helpers.reconciliation_path(@statement),
                            class: "inline-flex h-[30px] items-center rounded-lg border border-border px-3 text-[12.5px] font-medium text-muted-foreground hover:text-foreground no-underline transition-colors",
                            data: { turbo_frame: "_top" })
          end
        when :nif
          if item.transaction && @statement
            helpers.button_to(t(".ask_for_invoice"),
                              helpers.request_invoice_reconciliation_bank_transaction_path(@statement, item.transaction, surface: "money"),
                              class: "inline-flex h-[30px] items-center rounded-lg border border-border px-3 text-[12.5px] font-medium text-muted-foreground hover:text-foreground transition-colors cursor-pointer",
                              method: :post)
          end
        end
      end

      def overflow_note
        div(class: "mt-2 text-[12.5px] text-muted-foreground") do
          if @statement
            plain t(".overflow", count: @overflow)
            plain " "
            plain helpers.link_to(t(".overflow_link"), helpers.reconciliation_path(@statement),
                                  class: "underline underline-offset-2", data: { turbo_frame: "_top" })
          end
        end
      end
    end
  end
end
