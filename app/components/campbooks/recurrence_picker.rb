# frozen_string_literal: true

module Campbooks
  # A curated recurrence <select> — "Does not repeat" plus the Recurrence::PRESETS
  # — that stores a raw RRULE string. Shared by the calendar-event and task forms
  # so both offer the same repeat options. Values come from Recurrence::PRESETS;
  # labels from the shared `recurrence.*` i18n namespace (so a rule can be named
  # the same way wherever it appears). Composes Campbooks::Select.
  class RecurrencePicker < Campbooks::Base
    # @param name [String] form field name, e.g. "task[rrule]" or "calendar_event[rrule]"
    # @param selected [String, Recurrence, nil] the current rrule (raw or wrapped)
    # @param label [String, nil] overrides the default "Repeat" label
    def initialize(name:, selected: nil, label: nil)
      @name = name
      @selected = Recurrence.wrap(selected).rrule
      @label = label
    end

    def view_template
      render Campbooks::Select.new(
        @name,
        label: @label || t("recurrence.field_label"),
        options: options,
        selected: @selected,
        include_blank: t("recurrence.none"),
        data: { testid: "recurrence-picker" }
      )
    end

    private

    def options
      Recurrence.preset_options.map { |key, rrule| [ t("recurrence.presets.#{key}"), rrule ] }
    end
  end
end
