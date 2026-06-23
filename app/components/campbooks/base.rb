require "tailwind_merge"

module Campbooks
  class Base < Phlex::HTML
    include EmailMessageHelpers

    # One shared, Tailwind-aware merger so caller overrides actually win
    # (e.g. passing `class: "rounded-none"` resolves against a component's
    # default `rounded-lg` instead of both landing in the class list).
    TAILWIND_MERGER = ::TailwindMerge::Merger.new.freeze

    # Shared helper to merge Tailwind classes, filtering out nil/false/empty,
    # then resolving conflicts so the last-specified utility wins.
    #
    # `hidden` is special-cased: it's a visibility toggle, not a layout choice.
    # In the compiled CSS `.hidden` always wins over `.flex`/`.block`/`.grid`, so
    # the only way to reveal an element is to *remove* the class (usually via JS).
    # TailwindMerge, though, treats them as one `display` group and would drop a
    # bare `hidden` whenever a layout display utility follows it — silently
    # turning "hidden by default, shown on toggle" markup into always-visible
    # markup. So if the input asked for `hidden` and the merge dropped it, we
    # re-append it (where it renders identically to the CSS's own precedence).
    def self.class_names(*tokens)
      list = tokens.flatten.compact.reject(&:blank?)
      return "" if list.empty?

      merged = TAILWIND_MERGER.merge(list.join(" "))

      if wants_hidden?(list) && !token?(merged, "hidden")
        "#{merged} hidden"
      else
        merged
      end
    end

    def self.wants_hidden?(list)
      list.any? { |token| token?(token, "hidden") }
    end

    def self.token?(string, token)
      string.split(/\s+/).include?(token)
    end

    def class_names(*tokens)
      self.class.class_names(*tokens)
    end

    # === i18n ===
    # Component-scoped translation. A leading-dot key resolves under this
    # component's namespace — Campbooks::SkimCard#t(".keep") becomes
    # "components.skim_card.keep" — while any other key is treated as absolute, so
    # components can still reach shared keys (t("components.shared.close")) or any
    # app-level key. Delegates to the view's helpers, so HTML-safety, count-based
    # pluralization and the active locale behave exactly as they do in ERB.
    def t(key, **options)
      key = "#{self.class.i18n_scope}#{key}" if key.to_s.start_with?(".")
      helpers.t(key, **options)
    end

    # Locale-aware date/time/number formatting, e.g. l(time, format: :thread).
    def l(value, **options)
      helpers.l(value, **options)
    end

    # A readable text color (near-black or white) to place ON a solid background
    # hex, chosen by perceived luminance (YIQ). Keeps colored chips legible
    # whatever hue a provider assigned — e.g. white text on a pale calendar color
    # would otherwise vanish and fail WCAG AA.
    def contrast_on(hex)
      h = hex.to_s.delete("#")
      return "#ffffff" unless h.length == 6
      r = h[0, 2].to_i(16)
      g = h[2, 2].to_i(16)
      b = h[4, 2].to_i(16)
      ((r * 299) + (g * 587) + (b * 114)) / 1000 >= 140 ? "#1c1c1c" : "#ffffff"
    end

    # Campbooks::SkimCard -> "components.skim_card" (memoized per component class).
    def self.i18n_scope
      @i18n_scope ||= "components.#{name.delete_prefix("Campbooks::").underscore.tr("/", ".")}"
    end

    # Override to register assets if needed.
    # def self.register_assets
    # end
  end
end
