# frozen_string_literal: true

class ColorSwatchPickerPreview < Lookbook::Preview
  # The full event-color palette with the leading "none / inherit" option.
  def default
    render(Campbooks::ColorSwatchPicker.new(name: "calendar_event[color]"))
  end

  # A color pre-selected (Banana) — the checked swatch shows a ring.
  def selected
    render(Campbooks::ColorSwatchPicker.new(name: "calendar_event[color]", selected: "#fbd75b"))
  end

  # The none option relabelled for the calendar event form ("Calendar color").
  def with_inherit_label
    render(Campbooks::ColorSwatchPicker.new(name: "calendar_event[color]", none_label: "Calendar color"))
  end

  # Without the leading "none" option (a color is then required).
  def without_none
    render(Campbooks::ColorSwatchPicker.new(name: "calendar_event[color]", include_none: false))
  end
end
