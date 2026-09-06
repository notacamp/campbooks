# frozen_string_literal: true

module Campbooks
  module Money
    # "Not on a statement": invoices and receipts dated in a reconciled month that
    # have no bank line to show for them. Hidden until at least one statement is
    # reconciled; with statements but nothing missing it renders one quiet line.
    class Unbanked < Campbooks::Base
      def initialize(obligations:, evidence:, **attrs)
        @obligations = obligations
        @evidence    = evidence
        @attrs       = attrs
      end

      def view_template
        return unless @evidence.any?

        div(class: @attrs.delete(:class), **@attrs) do
          section_header
          if @obligations.empty?
            p(class: "text-[13.5px] text-muted-foreground") { t(".all_on_statement") }
          else
            div(class: "divide-y divide-border/50") { @obligations.each { |ob| obligation_row(ob) } }
            p(class: "mt-5 max-w-[62ch] text-[12px] leading-relaxed text-muted-foreground/80") { t(".evidence_note") }
          end
        end
      end

      private

      def section_header
        div(class: "mb-1 flex flex-wrap items-baseline gap-2") do
          span(class: "text-[11px] font-bold uppercase tracking-widest text-muted-foreground") do
            plain t(".title")
            span(class: "ml-1.5 font-normal normal-case tracking-normal text-muted-foreground/70") { plain "· #{@obligations.size}" }
          end
          span(class: "ml-auto text-[12px] text-muted-foreground") { t(".note") }
        end
      end

      def obligation_row(ob)
        div(id: ob.dom_id, class: "-mx-4 flex flex-wrap items-start justify-between gap-x-4 gap-y-2 rounded-xl px-4 py-3.5 transition-colors hover:bg-muted/50 sm:-mx-6 sm:px-6") do
          div(class: "min-w-0 flex-1") do
            div(class: "text-[14px] font-semibold text-foreground") { plain ob.counterpart.to_s }
            div(class: "mt-0.5 flex flex-wrap items-center gap-x-1.5 text-[12.5px] text-muted-foreground") do
              meta_parts(ob).each_with_index do |part, i|
                span(class: "mx-0.5 opacity-40", aria_hidden: "true") { plain "·" } if i.positive?
                span { plain part.to_s }
              end
            end
          end
          # Phones: the amount and the actions take their own line under the text.
          div(class: "flex w-full flex-wrap items-center justify-between gap-3 sm:w-auto sm:shrink-0 sm:justify-end") do
            amount_cell(ob)
            action_row(ob)
          end
        end
      end

      def meta_parts(ob)
        parts = []
        parts << ob.what.to_s if ob.what.present?
        parts << l(ob.anchor_on, format: :date) if ob.anchor_on
        parts << t(".not_on_label", label: ob.statement_label) if ob.statement_label
        parts << t(".you_sent") if ob.receivable?
        parts
      end

      def amount_cell(ob)
        css = class_names("text-[14px] font-semibold tabular-nums", ob.receivable? ? "text-success" : "text-foreground")
        span(class: css) { plain "#{ob.receivable? ? '+' : ''}#{ob.amount&.format}" }
      end

      def action_row(ob)
        div(class: "flex flex-wrap gap-1.5") do
          if ob.payable?
            post_form(helpers.money_obligation_settle_path(ob.id), hidden: { source: "elsewhere" }) do
              render(Campbooks::Button.new(variant: :outline, size: :sm, type: "submit")) { t(".paid_elsewhere") }
            end
          elsif ob.receivable?
            post_form(helpers.money_obligation_chase_path(ob.id)) do
              render(Campbooks::Button.new(variant: :outline, size: :sm, type: "submit")) { t(".send_reminder") }
            end
          end
          post_form(helpers.money_obligation_settle_path(ob.id)) do
            render(Campbooks::Button.new(variant: :outline, size: :sm, type: "submit")) { t(".mark_paid") }
          end
        end
      end

      def post_form(action, hidden: {})
        form(action: action, method: :post, class: "inline-flex") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          hidden.each { |name, value| input(type: "hidden", name: name.to_s, value: value.to_s) }
          yield
        end
      end
    end
  end
end
