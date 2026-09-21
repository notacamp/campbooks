# frozen_string_literal: true

module Api
  module App
    module Calendar
      # POST /api/app/calendar_import
      # Accepts a multipart .ics file upload and imports it into a writable calendar
      # via Calendars::IcsImporter. Returns a result summary.
      class CalendarImportController < Api::App::BaseController
        MAX_FILE_BYTES = 2.megabytes

        def create
          calendar = importable_calendars.find_by(id: params[:calendar_id])
          unless calendar
            return render_error("calendar_not_writable",
                                "The specified calendar is not writable or does not exist.",
                                status: :unprocessable_entity)
          end

          file = params[:ics_file]
          unless file.respond_to?(:read)
            return render_error("missing_file", "No ICS file provided.", status: :unprocessable_entity)
          end

          content = file.read
          if content.bytesize > MAX_FILE_BYTES
            return render_error("file_too_large",
                                "File exceeds the #{MAX_FILE_BYTES / 1.megabyte} MB limit.",
                                status: :unprocessable_entity)
          end

          result = Calendars::IcsImporter.new(calendar: calendar).call(content)
          render_data(
            Api::App::Calendar::CalendarImportResultSerializer.new(result).as_json,
            status: :created
          )
        rescue StandardError => e
          Rails.logger.error("[Api::App::Calendar::CalendarImport] import failed: #{e.class}: #{e.message}")
          render_error("parse_failed", "Failed to parse the ICS file.", status: :unprocessable_entity)
        end

        private

        def importable_calendars
          ::Calendar.where(
            calendar_account: current_user.writable_calendar_accounts,
            is_writable: true,
            syncing: true
          )
        end
      end
    end
  end
end
