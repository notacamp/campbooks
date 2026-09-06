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
        when :reconcile_statements
          reconcile_statements_row(item.payload)
        when :add_statement
          add_statement_row(item.payload)
        when :statement_failed
          statement_failed_row(item.payload[:reconciliation])
        else
          div(**wrapper_attrs(item)) do
            work_row(title: item.title, meta: item.meta) { actions(item) }
          end
        end
      end

      # Statements Scout already holds (emailed, filed) that nobody reconciled:
      # one click reconciles them all in the background, or pick which.
      def reconcile_statements_row(payload)
        docs  = payload[:documents]
        count = payload[:count]
        meta  = docs.first(3).map { |d| "#{d.display_title} · #{l(d.created_at.to_date, format: :date)}" }
        meta << t(".and_more", count: count - 3) if count > 3

        div(class: WRAPPER_CLASSES) do
          work_row(title: t(".reconcile_title", count: count), meta: meta) do
            render Campbooks::Button.new(variant: :outline, size: :sm, href: helpers.new_reconciliation_path,
                                         data: { turbo_frame: "_top" }) { t(".pick_which") }
            post_form(helpers.reconcile_statements_money_path) do
              render(Campbooks::Button.new(variant: :primary, size: :sm, type: "submit")) { t(".reconcile_them", count: count) }
            end
          end
        end
      end

      # A statement that couldn't be read: open it, or read it again.
      def statement_failed_row(recon)
        label = recon.period_label.presence || recon.statement_document&.display_title.presence || t(".statement_failed_untitled")
        meta  = [ recon.parse_error.to_s.truncate(110).presence, l(recon.created_at.to_date, format: :date) ].compact

        div(class: WRAPPER_CLASSES) do
          work_row(title: t(".statement_failed_title", label: label), meta: meta) do
            render Campbooks::Button.new(variant: :outline, size: :sm, href: helpers.reconciliation_path(recon),
                                         data: { turbo_frame: "_top" }) { t(".open_statement") }
            post_form(helpers.retry_parse_reconciliation_path(recon), hidden: { surface: "money" }) do
              render(Campbooks::Button.new(variant: :primary, size: :sm, type: "submit")) { t(".try_again") }
            end
          end
        end
      end

      # The month to reconcile has no statement yet, and Scout holds none.
      def add_statement_row(payload)
        div(class: WRAPPER_CLASSES) do
          work_row(title: t(".add_statement_title", month: payload[:label]), meta: [ t(".add_statement_meta") ]) do
            render Campbooks::Button.new(variant: :primary, size: :sm, href: helpers.new_reconciliation_path,
                                         data: { turbo_frame: "_top" }) { t(".add_statement") }
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
