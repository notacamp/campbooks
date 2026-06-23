# frozen_string_literal: true

module Calendars
  # The fixed palette of per-event colors, shared by the picker UI and both
  # directions of provider sync.
  #
  # Google Calendar events don't take an arbitrary hex — they carry a `colorId`
  # (1–11) drawn from a fixed palette. To round-trip faithfully we adopt that same
  # palette as the canonical set: an event's stored `color` is always one of these
  # hexes, so it maps cleanly to a Google `colorId` on write and back on read.
  # Zoho accepts a hex `color` directly, so the same value passes straight through.
  #
  # Hex values are the backgrounds Google's `colors.get` API returns for event
  # colors; `id` is the `colorId` Google expects on write.
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

    BY_ID  = PALETTE.index_by { |c| c[:id] }.freeze
    BY_HEX = PALETTE.index_by { |c| c[:hex] }.freeze

    module_function

    # The ordered palette for the picker. Each entry is { id:, hex:, name: }.
    def palette
      PALETTE
    end

    # Google colorId (String or Integer) → our hex, or nil for an unknown/blank id
    # (event has no override → caller falls back to the calendar color).
    def hex_for(color_id)
      BY_ID[color_id.to_s]&.fetch(:hex)
    end

    # Our hex → Google colorId, or nil when the hex isn't in the palette (e.g. a
    # blank/inherited color). A nil colorId on write resets the event to the
    # calendar's default color.
    def id_for(hex)
      return nil if hex.blank?
      BY_HEX[hex.to_s.downcase]&.fetch(:id)
    end
  end
end
