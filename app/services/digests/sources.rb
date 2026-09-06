# frozen_string_literal: true

module Digests
  # Registry for digest source types. Each source gathers items from one domain
  # (emails, calendar events, tasks, reminders, or documents) for a given period.
  module Sources
    KEYS = %w[emails calendar tasks reminders documents].freeze

    # Resolve a source class by type key.
    def self.for(type)
      case type.to_s
      when "emails"    then Sources::Emails
      when "calendar"  then Sources::Calendar
      when "tasks"     then Sources::Tasks
      when "reminders" then Sources::Reminders
      when "documents" then Sources::Documents
      end
    end

    # Source keys available for a given workspace. Excludes the tasks (asks) source
    # only when the readiness flag is off (ENABLE_TASKS=0); asks are no longer a paid
    # feature, so no entitlement is checked (the workspace arg is kept for the
    # signature its callers already use).
    def self.available_keys(_workspace)
      KEYS.reject do |key|
        key == "tasks" && !Features.tasks?
      end
    end
  end
end
