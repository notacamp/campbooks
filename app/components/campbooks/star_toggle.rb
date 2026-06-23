# frozen_string_literal: true

module Campbooks
  # Star / unstar toggle for a record (documents today; reusable anywhere a row can be
  # flagged "important"). Posts a PATCH to `url` as a Turbo form and is swapped in place
  # via Turbo Stream, so the icon flips without re-sorting the list. Starred renders a
  # filled amber star; unstarred a faint outline that warms on hover.
  class StarToggle < Campbooks::Base
    # @param url [String] PATCH endpoint that toggles the starred state
    # @param starred [Boolean] current state
    # @param size [Symbol] :sm (lists, default) or :md (detail headers)
    # @param label [String, nil] accessible label override (defaults to Star/Unstar)
    def initialize(url:, starred:, size: :sm, label: nil)
      @url = url
      @starred = starred
      @size = size
      @label = label
    end

    def view_template
      form(action: @url, method: "post", class: "inline-flex flex-shrink-0") do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token, autocomplete: "off")
        input(type: "hidden", name: "_method", value: "patch")

        button(
          type: "submit",
          class: button_classes,
          title: aria_label,
          aria_label: aria_label,
          aria_pressed: @starred.to_s
        ) do
          raw(safe(star_svg))
        end
      end
    end

    private

    def aria_label
      @label || (@starred ? t(".unstar") : t(".star"))
    end

    def button_classes
      class_names(
        "inline-flex items-center justify-center rounded-md cursor-pointer transition-colors",
        @size == :md ? "p-1.5" : "p-1",
        (@starred ? "text-amber-400 hover:text-amber-500" : "text-gray-300 hover:text-amber-400")
      )
    end

    def icon_class
      @size == :md ? "w-5 h-5" : "w-4 h-4"
    end

    def star_svg
      if @starred
        %(<svg class="#{icon_class}" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path fill-rule="evenodd" clip-rule="evenodd" d="#{SOLID_PATH}"/></svg>)
      else
        %(<svg class="#{icon_class}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="#{OUTLINE_PATH}"/></svg>)
      end
    end

    # Heroicons v2 "star" (24×24) — solid for the on state, outline for the off state.
    SOLID_PATH = "M10.788 3.21c.448-1.077 1.976-1.077 2.424 0l2.082 5.007 5.404.433c1.164.093 1.636 1.545.749 2.305l-4.117 3.527 1.257 5.273c.271 1.136-.964 2.033-1.96 1.425L12 18.354 7.373 21.18c-.996.608-2.231-.29-1.96-1.425l1.257-5.273-4.117-3.527c-.887-.76-.415-2.212.749-2.305l5.404-.433 2.082-5.006z"
    OUTLINE_PATH = "M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.562.562 0 01-.84-.61l1.285-5.386a.562.562 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z"
  end
end
