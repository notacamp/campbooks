# frozen_string_literal: true

module Campbooks
  # A radio-group of color swatches bound to a form field. The leading option is a
  # "none / inherit" choice that submits a blank value — the calendar event form
  # uses it so an event can fall back to its calendar's color (CalendarEvent#display_color).
  #
  # Swatches are real radio inputs (visually hidden via `sr-only peer`) so the
  # control stays keyboard- and screen-reader-accessible; the checked swatch shows
  # a ring driven by `peer-checked:` (the visible swatch is the input's next
  # sibling, as the `peer` selector requires).
  #
  # @param name [String] form field name, e.g. "calendar_event[color]"
  # @param selected [String, nil] currently-selected hex (blank → the none option)
  # @param colors [Array<Hash>, nil] swatches as { hex:, name: }; defaults to the event palette
  # @param include_none [Boolean] render the leading "none / inherit" option
  # @param none_label [String] accessible label + tooltip for the none option
  class ColorSwatchPicker < Campbooks::Base
    SWATCH = "flex h-7 w-7 items-center justify-center rounded-full ring-1 ring-black/10 transition " \
             "peer-checked:ring-2 peer-checked:ring-foreground peer-checked:ring-offset-2 peer-checked:ring-offset-card " \
             "peer-focus-visible:ring-2 peer-focus-visible:ring-ring peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-card"

    def initialize(name:, selected: nil, colors: nil, include_none: true, none_label: "None")
      @name = name
      @selected = selected.to_s
      @colors = colors || Calendars::EventColors.palette
      @include_none = include_none
      @none_label = none_label
    end

    def view_template
      div(class: "flex flex-wrap items-center gap-2", role: "radiogroup", aria_label: @none_label) do
        none_option if @include_none
        @colors.each { |c| color_option(c[:hex], c[:name]) }
      end
    end

    private

    def color_option(hex, text)
      option(value: hex, text: text) do
        span(class: SWATCH, style: "background-color: #{hex}")
      end
    end

    # "None": a neutral circle with a diagonal slash, signalling "no fixed color —
    # inherit the calendar's".
    def none_option
      option(value: "", text: @none_label) do
        span(class: "#{SWATCH} bg-card") do
          raw(safe(%(<svg viewBox="0 0 24 24" class="h-5 w-5 text-muted-foreground" aria-hidden="true"><line x1="5" y1="19" x2="19" y2="5" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>)))
        end
      end
    end

    def option(value:, text:)
      label(class: "cursor-pointer", title: text) do
        input(type: "radio", name: @name, value: value, checked: @selected.casecmp?(value), class: "sr-only peer", aria_label: text)
        yield
      end
    end
  end
end
