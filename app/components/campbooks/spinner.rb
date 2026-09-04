# frozen_string_literal: true

module Campbooks
  class Spinner < Campbooks::Base
    # @param size [Symbol] :sm (w-4 h-4), :md (w-5 h-5), :lg (w-8 h-8)
    def initialize(size: :md, **attrs)
      @size = size
      @attrs = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)
      merged = class_names(
        "animate-spin border-gray-300 border-t-accent-500 rounded-full",
        SIZE_CLASSES[@size],
        custom_class
      )

      # The status wrapper is positioned so the screen-reader-only label (which
      # Tailwind's sr-only makes absolutely positioned) stays inside it. Without a
      # positioned ancestor the label anchors to the page at the spinner's in-flow
      # position — inside a scroll pane (the People list's infinite-scroll sentinel)
      # that lands far below the viewport and stretches the document, so wheeling
      # over the pane border scrolled the whole page into blank space.
      div(role: "status", class: "relative", **@attrs) do
        div(class: merged)
        span(class: "sr-only") { t("shared.actions.loading") }
      end
    end

    private

    SIZE_CLASSES = {
      sm: "w-4 h-4 border-2",
      md: "w-5 h-5 border-2",
      lg: "w-8 h-8 border-[3px]"
    }.freeze
  end
end
