# frozen_string_literal: true

module Campbooks
  # A radio-group of selectable icons bound to a form field, modeled on
  # ColorSwatchPicker: real radio inputs (visually hidden via `sr-only peer`) so
  # the control stays keyboard- and screen-reader-accessible, with the checked
  # tile lit by `peer-checked:` — no JavaScript.
  #
  # The leading option is "default", which submits a blank value so the folder
  # falls back to the default folder glyph (see MailFolder#display_icon).
  #
  # @param name [String] form field name, e.g. "mail_folder[icon]"
  # @param selected [String, nil] currently-selected icon name (blank → default)
  class IconPicker < Campbooks::Base
    TILE = "flex h-9 w-9 items-center justify-center rounded-lg text-muted-foreground ring-1 ring-border transition " \
           "hover:bg-muted peer-checked:bg-accent-600 peer-checked:text-white peer-checked:ring-accent-600 " \
           "peer-focus-visible:ring-2 peer-focus-visible:ring-ring peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-card"

    def initialize(name:, selected: nil)
      @name = name
      @selected = selected.to_s
    end

    def view_template
      div(class: "flex flex-wrap gap-2", role: "radiogroup", aria_label: t(".label")) do
        icon_option("", t(".default"), Campbooks::Icon::DEFAULT)
        Campbooks::Icon::NAMES.each { |icon| icon_option(icon, icon.tr("-", " "), icon) }
      end
    end

    private

    # @param value [String] submitted value ("" → default glyph)
    # @param label [String] accessible label + tooltip
    # @param glyph [String] icon name to draw in the tile
    def icon_option(value, label, glyph)
      label(class: "cursor-pointer", title: label) do
        input(type: "radio", name: @name, value: value, checked: @selected.casecmp?(value), class: "sr-only peer", aria_label: label)
        span(class: TILE) { render(Campbooks::Icon.new(glyph, css_class: "w-5 h-5")) }
      end
    end
  end
end
