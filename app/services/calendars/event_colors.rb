# frozen_string_literal: true

module Calendars
  # The fixed color palette behind Campbooks::ColorSwatchPicker — today that's
  # the calendar color picker in the /calendar sidebar (events no longer carry
  # a color of their own; the calendar is the single color source).
  #
  # The hexes are the backgrounds Google's `colors.get` API returns for event
  # colors — kept as a pleasant, well-spaced set even though we no longer map
  # them to Google colorIds (per-event color sync was removed with the
  # calendar-owned-color model).
  module EventColors
    PALETTE = [
      { id: "1",  hex: "#a4bdfc", name: "Lavender" },
      { id: "2",  hex: "#7ae7bf", name: "Sage" },
      { id: "3",  hex: "#dbadff", name: "Grape" },
      { id: "4",  hex: "#ff887c", name: "Flamingo" },
      { id: "5",  hex: "#fbd75b", name: "Banana" },
      { id: "6",  hex: "#ffb878", name: "Tangerine" },
      { id: "7",  hex: "#46d6db", name: "Peacock" },
      { id: "8",  hex: "#e1e1e1", name: "Graphite" },
      { id: "9",  hex: "#5484ed", name: "Blueberry" },
      { id: "10", hex: "#51b749", name: "Basil" },
      { id: "11", hex: "#dc2127", name: "Tomato" }
    ].freeze

    module_function

    # The ordered palette for the picker. Each entry is { id:, hex:, name: }.
    def palette
      PALETTE
    end
  end
end
