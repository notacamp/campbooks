# frozen_string_literal: true

module Campbooks
  module People
    # The round icon tile for a stream, keyed by kind (mail / bell / file / users).
    # Owns the icon set so both the stream row and the stream-detail header render
    # it the same way (and neither has to `raw` an SVG in a view).
    class StreamIcon < Campbooks::Base
      ICONS = {
        mail:  '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/>',
        bell:  '<path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/>',
        file:  '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/>',
        users: '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>'
      }.freeze

      def initialize(kind:)
        @kind = kind.to_sym
      end

      def view_template
        div(class: "flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-secondary text-muted-foreground") do
          svg(class: "h-[18px] w-[18px]", fill: "none", stroke: "currentColor", "stroke-width": "1.9",
              "stroke-linecap": "round", "stroke-linejoin": "round", viewBox: "0 0 24 24", "aria-hidden": "true") do
            raw(safe(ICONS.fetch(@kind, ICONS[:mail])))
          end
        end
      end
    end
  end
end
