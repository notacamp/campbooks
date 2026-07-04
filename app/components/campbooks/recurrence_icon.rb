# frozen_string_literal: true

module Campbooks
  # The small "repeat" glyph shown next to a recurring event or task. Rendered as a
  # raw inline SVG (matching the other icon call-sites in the calendar/task
  # components) with an accessible label so screen readers announce the recurrence.
  class RecurrenceIcon < Campbooks::Base
    def initialize(css: "w-3.5 h-3.5 text-gray-400 shrink-0", label: nil)
      @css = css
      @label = label
    end

    def view_template
      raw(safe(markup))
    end

    private

    def markup
      title = ERB::Util.html_escape(@label || t("recurrence.icon_label"))
      <<~SVG.squish
        <svg class="#{ERB::Util.html_escape(@css)}" viewBox="0 0 24 24" fill="none"
             stroke="currentColor" stroke-width="2" role="img" aria-label="#{title}">
          <title>#{title}</title>
          <path d="M17 2l4 4-4 4"/><path d="M3 11v-1a4 4 0 014-4h14"/>
          <path d="M7 22l-4-4 4-4"/><path d="M21 13v1a4 4 0 01-4 4H3"/>
        </svg>
      SVG
    end
  end
end
