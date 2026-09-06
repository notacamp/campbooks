# frozen_string_literal: true

module Campbooks
  module Money
    # "Needs you": the lines of the newest statement that want a decision, plus the
    # loan's alerts and Scout's untracked-loan guess, as flat rows (Ember dot, title,
    # meta, right-aligned actions). Hidden when empty.
    #
    # A "No invoice" row carries the same transaction-resolve Stimulus contract as
    # the reconciliation page, so Resolve opens the hunt panel in place: the row's
    # root is a block wrapper (id = dom_id(txn)) around the flex row, and the panel
    # the controller appends lands below the row inside that wrapper.
    class NeedsYou < Campbooks::Base
      include Campbooks::Money::WorkRowShell

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
            plain t(".title")
            span(class: "ml-1.5 font-normal normal-case tracking-normal text-muted-foreground/70") { plain "· #{@items.size}" }
          end
          span(class: "ml-auto text-[12px] text-muted-foreground") { t(".note") } if @statement
        end
      end

      def row(item)
        case item.kind
        when :loan_suggestion
          render Campbooks::Money::LoanSuggestionRow.new(suggestion: item.payload)
        when :loan_missed, :loan_changed
          render Campbooks::Money::LoanAlertRow.new(loan: item.payload[:loan], instalment: item.payload[:instalment],
                                                    kind: item.kind == :loan_missed ? :missed : :changed)
        else
          div(**wrapper_attrs(item)) do
            work_row(title: item.title, meta: item.meta) { actions(item) }
          end
        end
      end

      def wrapper_attrs(item)
        attrs = { class: WRAPPER_CLASSES }
        return attrs unless item.kind == :no_invoice && item.transaction && @statement

        txn = item.transaction
        attrs[:id]   = helpers.dom_id(txn)
        attrs[:data] = {
          controller:                         "transaction-resolve",
          transaction_resolve_url_value:      helpers.resolve_panel_reconciliation_bank_transaction_path(@statement, txn, surface: "money"),
          transaction_resolve_frame_id_value: helpers.dom_id(txn, :resolve_frame)
        }
        attrs
      end

      def actions(item)
        case item.kind
        when :no_invoice
          return unless item.transaction && @statement

          button(type: "button", class: button_classes(:outline), data: { action: "click->transaction-resolve#toggle" }) { t(".resolve") }
        when :review
          return unless item.transaction && @statement

          render Campbooks::Button.new(variant: :outline, size: :sm,
                                       href: helpers.reconciliation_path(@statement, anchor: helpers.dom_id(item.transaction)),
                                       data: { turbo_frame: "_top" }) { t(".change") }
          if item.match
            post_form(helpers.confirm_line_money_path(item.transaction.id), hidden: { match_id: item.match.id }) do
              render(Campbooks::Button.new(variant: :primary, size: :sm, type: "submit")) { t(".confirm") }
            end
          end
        when :partial
          return unless @statement

          render Campbooks::Button.new(variant: :outline, size: :sm, href: helpers.reconciliation_path(@statement),
                                       data: { turbo_frame: "_top" }) { t(".open_statement") }
        when :nif
          return unless item.transaction && @statement

          post_form(helpers.request_invoice_reconciliation_bank_transaction_path(@statement, item.transaction),
                    hidden: { surface: "money" }) do
            render(Campbooks::Button.new(variant: :outline, size: :sm, type: "submit")) { t(".ask_for_invoice") }
          end
        end
      end

      def overflow_note
        return unless @statement

        div(class: "mt-2 text-[12.5px] text-muted-foreground") do
          plain t(".overflow", count: @overflow)
          whitespace
          a(href: helpers.reconciliation_path(@statement), class: "underline underline-offset-2 hover:text-foreground",
            data: { turbo_frame: "_top" }) { t(".overflow_link") }
        end
      end
    end
  end
end
