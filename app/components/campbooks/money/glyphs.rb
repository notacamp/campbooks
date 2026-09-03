# frozen_string_literal: true

module Campbooks
  module Money
    # The stroked line-icons the Money timeline and ledger share, lifted from the
    # approved mock (stroke-width 1.9, round caps/joins). `warning` is the one glyph
    # the mock set didn't carry — a standard triangle-alert — used on every late
    # marker so lateness never rides on colour alone.
    module Glyphs
      ICONS = {
        mail:    '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/>',
        file:    '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/>',
        check:   '<path d="M20 6 9 17l-5-5"/>',
        clock:   '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
        send:    '<path d="m22 2-7 20-4-9-9-4z"/><path d="M22 2 11 13"/>',
        spark:   '<path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/>',
        warning: '<path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><path d="M12 9v4M12 17h.01"/>',
        money:   '<rect x="2" y="6" width="20" height="12" rx="2"/><circle cx="12" cy="12" r="2.5"/><path d="M6 12h.01M18 12h.01"/>'
      }.freeze

      # A filled spark (Scout's mark) reuses the fill treatment; everything else is stroked.
      def money_icon(name, css_class: "h-4 w-4", fill: false)
        attrs = if fill
          %(viewBox="0 0 24 24" fill="currentColor")
        else
          %(viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round")
        end
        raw safe(%(<svg class="#{css_class}" #{attrs} aria-hidden="true">#{ICONS.fetch(name)}</svg>))
      end
    end
  end
end
