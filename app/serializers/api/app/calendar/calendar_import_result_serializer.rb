# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Serializes the result of a Calendars::IcsImporter#call for POST /api/app/calendar_import.
      class CalendarImportResultSerializer
        def initialize(result)
          @result = result
        end

        def as_json
          {
            imported: @result.imported,
            skipped_recurring: @result.skipped_recurring,
            skipped_duplicate: @result.skipped_duplicate,
            skipped_malformed: @result.skipped_malformed,
            truncated: @result.truncated
          }
        end
      end
    end
  end
end
