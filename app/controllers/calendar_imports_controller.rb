# One-shot .ics import: pick a file and a writable calendar, and
# Calendars::IcsImporter creates the events locally — each then pushes out to
# the provider through the normal outbound write path, like manually created
# events. A rare flow, so a small standalone page (linked from the calendar
# sidebar) rather than a modal. The upload is parsed and discarded, never stored.
class CalendarImportsController < ApplicationController
  before_action :require_authentication

  MAX_FILE_BYTES = 2.megabytes
  ICS_CONTENT_TYPES = %w[text/calendar application/ics].freeze

  def new
    @calendars = importable_calendars
    redirect_to calendar_path, error: t(".no_writable_calendars") if @calendars.empty?
  end

  def create
    calendar = importable_calendars.find_by(id: params[:calendar_id])
    return redirect_to(new_calendar_import_path, error: t(".no_calendar")) unless calendar

    file = params[:ics_file]
    return redirect_to(new_calendar_import_path, error: t(".no_file")) unless file.respond_to?(:tempfile)
    return redirect_to(new_calendar_import_path, error: t(".too_large")) if file.size.to_i > MAX_FILE_BYTES
    return redirect_to(new_calendar_import_path, error: t(".wrong_type")) unless ics_file?(file)

    result = Calendars::IcsImporter.new(calendar: calendar).call(file.read)
    redirect_to calendar_path, success: summary_for(result)
  rescue StandardError => e
    Rails.logger.error("[CalendarImports] import failed: #{e.class}: #{e.message}")
    redirect_to new_calendar_import_path, error: t(".parse_failed")
  end

  private

  # Same bar as the event form's calendar picker: write-shared with this user,
  # provider-writable, and actively syncing.
  def importable_calendars
    Calendar.where(calendar_account: Current.user.writable_calendar_accounts,
                   is_writable: true, syncing: true)
            .includes(:calendar_account).order(is_primary: :desc, name: :asc)
  end

  def ics_file?(file)
    file.original_filename.to_s.downcase.end_with?(".ics") ||
      ICS_CONTENT_TYPES.include?(file.content_type.to_s)
  end

  # Absolute keys (not lazy `t(".…")`): this is called from #create, and lazy
  # lookup inside a helper method trips i18n-tasks' static scan.
  def summary_for(result)
    scope = "calendar_imports.create"
    parts = [ t("#{scope}.imported", count: result.imported) ]
    parts << t("#{scope}.skipped_recurring", count: result.skipped_recurring) if result.skipped_recurring.positive?
    parts << t("#{scope}.skipped_duplicate", count: result.skipped_duplicate) if result.skipped_duplicate.positive?
    parts << t("#{scope}.skipped_malformed", count: result.skipped_malformed) if result.skipped_malformed.positive?
    parts << t("#{scope}.truncated", max: Calendars::IcsImporter::MAX_EVENTS) if result.truncated
    parts.join(" ")
  end
end
