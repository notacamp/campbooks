# frozen_string_literal: true

module Campbooks
  module Feed
    # The lightest unit: a compact, borderless one-line filing suggestion —
    # "File <subject> under #tag" with a one-tap action. Tighter than every other
    # card so a run of them reads as a quick filing queue, not a wall of cards.
    class TagSuggestionCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex items-center gap-3 rounded-2xl px-3 py-2.5 transition-colors duration-150 hover:bg-muted/50") do
          icon_circle
          div(class: "min-w-0 flex-1 text-sm text-muted-foreground") do
            plain t(".prefix")
            whitespace
            span(class: "font-medium text-foreground") { clean_subject(subject).truncate(44, separator: " ") }
            whitespace
            plain t(".under")
            whitespace
            tag_chip
          end
          div(class: "flex flex-shrink-0 items-center gap-1.5") do
            act_button(tool: "add_tag", args: { tag_name: tag_name }, label: t(".file_it"), variant: :primary, size: :xs, key: "c", primary: true)
            dismiss_button(label: t(".not_now"), variant: :ghost, size: :xs, key: "x")
          end
        end
      end

      private

      def tag_name = item.data["tag_name"].to_s

      def tag_chip
        span(class: "inline-flex max-w-[160px] items-center rounded-md bg-muted px-2 py-0.5 align-bottom text-[12px] font-semibold text-foreground/80") do
          span(class: "truncate") { "##{tag_name}" }
        end
      end

      def icon_circle
        span(class: "flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground") do
          raw safe(tag_icon)
        end
      end

      def tag_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4"><path d="M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z"/><circle cx="7.5" cy="7.5" r=".5" fill="currentColor"/></svg>)
      end
    end
  end
end
