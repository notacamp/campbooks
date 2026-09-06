# frozen_string_literal: true

module Campbooks
  module Money
    # "Needs you": the lines of the newest statement that want a decision, as flat
    # rows (Ember dot, title, meta, right-aligned actions). Hidden when empty.
    #
    # A "No invoice" row carries the same transaction-resolve Stimulus contract as
    # the reconciliation page, so Resolve opens the hunt panel in place: the row's
    # root is a block wrapper (id = dom_id(txn)) around the flex row, and the panel
    # the controller appends lands below the row inside that wrapper.
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
            plain t(".title")
            span(class: "ml-1.5 font-normal normal-case tracking-normal text-muted-foreground/70") { plain "· #{@items.size}" }
          end
          span(class: "ml-auto text-[12px] text-muted-foreground") { t(".note") } if @statement
        end
      end

      def row(item)
        div(**wrapper_attrs(item)) do
          div(class: "flex flex-wrap items-start gap-3 rounded-xl px-4 py-3.5 transition-colors hover:bg-muted/50 sm:flex-nowrap sm:px-6") do
            span(class: "mt-[6px] h-2 w-2 shrink-0 rounded-full bg-ember-gradient shadow-ember-glow", aria_hidden: "true")
            div(class: "min-w-0 flex-1 basis-[calc(100%-1.25rem)] sm:basis-auto") do
              div(class: "text-[14px] font-semibold leading-snug text-foreground") { plain item.title }
              meta_line(item) if item.meta.present?
            end
            # Phones: actions take their own line, aligned under the title.
            div(class: "flex w-full flex-wrap items-center justify-end gap-2 pl-5 sm:w-auto sm:shrink-0 sm:pl-0") { actions(item) }
          end
        end
      end

      def wrapper_attrs(item)
        attrs = { class: "-mx-4 sm:-mx-6" }
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

      def meta_line(item)
        div(class: "mt-0.5 flex flex-wrap items-center gap-x-1.5 gap-y-0.5 text-[12.5px] text-muted-foreground") do
          item.meta.each_with_index do |part, i|
            span(class: "mx-0.5 opacity-40", aria_hidden: "true") { plain "·" } if i.positive?
            if part.to_s == t("money.needs_you.nif.flag")
              span(class: "rounded border border-warning/40 px-1 text-[10px] font-bold text-warning") { plain "NIF" }
            else
              span { plain part.to_s }
            end
          end
        end
      end

      def actions(item)
        case item.kind
        when :no_invoice
          return unless item.transaction && @statement

          button(type: "button", class: outline_classes, data: { action: "click->transaction-resolve#toggle" }) { t(".resolve") }
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

      def post_form(action, hidden: {})
        form(action: action, method: :post, class: "inline-flex") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          hidden.each { |name, value| input(type: "hidden", name: name.to_s, value: value.to_s) }
          yield
        end
      end

      def outline_classes
        class_names(Campbooks::Button::BASE_CLASSES, Campbooks::Button::VARIANT_CLASSES[:outline], Campbooks::Button::SIZE_CLASSES[:sm])
      end
    end
  end
end
