# frozen_string_literal: true

module Digests
  # Predefined digest templates. Each preset has a key, icon, source configuration
  # template, rrule, and schedule_hint (weekday + time). Human-facing label and
  # description are i18n'd under digests.presets.<key>.
  module Presets
    Preset = Data.define(:key, :icon, :sources, :rrule, :schedule_hint)

    ALL = [
      Preset.new(
        key:           "newsletter_roundup",
        icon:          "inbox",
        sources:       [ { "type" => "emails", "query" => "category:promotions category:updates" } ],
        rrule:         "FREQ=WEEKLY",
        schedule_hint: { wday: 1, hour: 8, min: 0 }   # Monday 08:00
      ),
      Preset.new(
        key:           "week_ahead",
        icon:          "calendar",
        sources:       [
          { "type" => "calendar", "window_days" => 7 },
          { "type" => "tasks",    "window_days" => 7, "include_overdue" => false },
          { "type" => "reminders", "window_days" => 7 }
        ],
        rrule:         "FREQ=WEEKLY",
        schedule_hint: { wday: 0, hour: 18, min: 0 }  # Sunday 18:00
      ),
      Preset.new(
        key:           "upcoming_tasks",
        icon:          "tasks",
        sources:       [ { "type" => "tasks", "window_days" => 7, "include_overdue" => true } ],
        rrule:         "FREQ=WEEKLY",
        schedule_hint: { wday: 1, hour: 7, min: 30 }  # Monday 07:30
      ),
      Preset.new(
        key:           "invoice_tracker",
        icon:          "documents",
        sources:       [ { "type" => "documents", "document_types" => [ "invoice", "receipt" ] } ],
        rrule:         "FREQ=WEEKLY",
        schedule_hint: { wday: 5, hour: 17, min: 0 }  # Friday 17:00
      ),
      Preset.new(
        key:           "client_pulse",
        icon:          "inbox",
        sources:       [ { "type" => "emails", "query" => "domain:example.com" } ],
        rrule:         "FREQ=WEEKLY",
        schedule_hint: { wday: 1, hour: 9, min: 0 }   # Monday 09:00
      ),
      Preset.new(
        key:           "custom",
        icon:          "custom",
        sources:       [ { "type" => "emails", "query" => "" } ],
        rrule:         "FREQ=WEEKLY",
        schedule_hint: { wday: 1, hour: 8, min: 0 }   # Monday 08:00
      )
    ].freeze

    def self.find(key)
      ALL.find { |p| p.key == key.to_s }
    end

    # Presets available for a workspace: tasks-dependent presets are filtered or
    # adjusted when the tasks source is not available.
    def self.all(workspace)
      tasks_available = Digests::Sources.available_keys(workspace).include?("tasks")

      ALL.filter_map do |preset|
        if preset.key == "upcoming_tasks"
          # Omit entirely when tasks are not available.
          next unless tasks_available
          preset
        elsif preset.key == "week_ahead" && !tasks_available
          # Drop only the tasks source; keep calendar + reminders.
          adjusted_sources = preset.sources.reject { |s| s["type"] == "tasks" }
          Preset.new(
            key:           preset.key,
            icon:          preset.icon,
            sources:       adjusted_sources,
            rrule:         preset.rrule,
            schedule_hint: preset.schedule_hint
          )
        else
          preset
        end
      end
    end
  end
end
