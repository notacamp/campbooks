# frozen_string_literal: true

module Campbooks
  # Scout's one-line contribution as a horizontal glass row (DESIGN.md §5 "Scout note"):
  # an Ember spark avatar, a bold "Scout" + "AI" tag, the read, and an optional trailing
  # action (yielded). Paper puts its document summary here; the trailing slot carries the
  # "Review it/them" button or the "Connect AI" link.
  #
  #   render Campbooks::ScoutNoteRow.new(text: summary.sentence) do
  #     render Campbooks::Button.new(variant: :outline, size: :sm, data: {...}) { "Review it" }
  #   end
  #
  # @param text [String] Scout's read (already localized)
  class ScoutNoteRow < Campbooks::Base
    def initialize(text:, **attrs)
      @text = text
      @attrs = attrs
    end

    def view_template(&action)
      div(class: class_names(
        "flex flex-wrap items-center gap-x-3 gap-y-2 rounded-[14px] px-3.5 py-2.5 text-[13.5px] leading-snug scout-glass",
        @attrs.delete(:class)
      ), **@attrs) do
        render Campbooks::ScoutAvatar.new(size: :xs)
        div(class: "min-w-0 flex-1") do
          span(class: "font-semibold text-foreground") { t("components.scout_note_row.name") }
          whitespace
          span(class: "inline-flex items-center rounded bg-ember-gradient px-1 py-px align-[1px] text-[9.5px] font-bold uppercase tracking-wide text-white") do
            t("components.scout_note_row.ai")
          end
          whitespace
          span(class: "text-foreground/85") { @text }
        end
        div(class: "shrink-0") { yield } if block_given?
      end
    end
  end
end
