# frozen_string_literal: true

# @label Recurrence Picker
class RecurrencePickerComponentPreview < ViewComponent::Preview
  # Nothing selected — the "Does not repeat" default.
  def default
    render Campbooks::RecurrencePicker.new(name: "record[rrule]")
  end

  # A preset pre-selected (edit of an existing weekly series).
  def weekly_selected
    render Campbooks::RecurrencePicker.new(name: "record[rrule]", selected: "FREQ=WEEKLY")
  end

  def monthly_selected
    render Campbooks::RecurrencePicker.new(name: "record[rrule]", selected: "FREQ=MONTHLY")
  end

  # A provider rule that isn't one of our presets still renders (falls through to
  # the blank prompt in the <select>; the label naming is handled elsewhere).
  def custom_rule
    render Campbooks::RecurrencePicker.new(name: "record[rrule]", selected: "FREQ=WEEKLY;BYDAY=TU,TH")
  end
end
