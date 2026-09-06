# frozen_string_literal: true

module Campbooks
  module Scout
    # The ask rows Scout renders under a chat answer to "what do I owe people?".
    # Each card (from Tools::QueryAsks, kind "ask") is one row: an ink-outline dot,
    # the title, a muted meta line ("by Friday · held Thu 10:00 · from Sofia's
    # email"), an Open link to the source (when present) and a Done button that
    # marks the ask done and drops just this row (return=scout). Wraps at 375px.
    class AskCards < Campbooks::Base
      def initialize(cards:)
        @cards = Array(cards)
      end

      def view_template
        return if @cards.empty?

        div(class: "mt-2 flex w-full max-w-[42rem] flex-col gap-1.5") do
          @cards.each { |card| card_row(card) }
        end
      end

      private

      def card_row(card)
        div(id: "ask_card_#{card['id']}",
            class: "flex flex-wrap items-center gap-x-2 gap-y-1 rounded-lg border border-border bg-card px-3 py-2") do
          dot
          span(class: "min-w-0 break-words text-[13px] font-medium text-foreground") { card["title"].to_s }
          meta(card["meta"]) if card["meta"].present?
          div(class: "ml-auto flex flex-shrink-0 items-center gap-1.5") do
            open_link(card["path"]) if card["path"].present?
            done_form(card["id"])
          end
        end
      end

      def dot
        span(class: "inline-block h-2.5 w-2.5 flex-shrink-0 rounded-[3px] border border-foreground/50")
      end

      def meta(text)
        span(class: "min-w-0 break-words text-[12px] text-muted-foreground") { text.to_s }
      end

      def open_link(path)
        a(href: path, data: { turbo_frame: "_top" },
          class: "rounded-md px-2 py-1 text-[12px] font-medium text-foreground no-underline hover:bg-muted") { t(".open") }
      end

      # Done marks the ask done (PATCH) and, via return=scout, AsksController drops
      # this row (id "ask_card_<id>") and raises a toast.
      def done_form(id)
        form(action: helpers.done_ask_path(id), method: :post, class: "inline-flex", data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "_method", value: "patch")
          input(type: "hidden", name: "return", value: "scout")
          button(type: :submit,
                 class: "rounded-md border border-input bg-background px-2 py-1 text-[12px] font-medium text-foreground hover:bg-accent") { t(".done") }
        end
      end
    end
  end
end
