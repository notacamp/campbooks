# Receives reports from the in-app "Report a bug" widget. The report is always
# persisted to the workspace (so nothing is lost), then mirrored to a GitHub
# issue by BugReportGithubSyncJob when GitHub is configured. The widget submits
# via fetch (JSON), but a plain <form> fallback (HTML) also works without JS.
class BugReportsController < ApplicationController
  def create
    @bug_report = Current.workspace.bug_reports.new(
      user: current_user,
      description: params[:description].to_s,
      page_url: params[:page_url].presence,
      user_agent: request.user_agent,
      metadata: captured_metadata
    )
    attach_screenshot

    if @bug_report.save
      BugReportGithubSyncJob.perform_later(@bug_report.id) if BugReportGithubSyncJob.configured?

      respond_to do |format|
        format.json { render json: { ok: true, id: @bug_report.id }, status: :created }
        format.html { redirect_back fallback_location: root_path, success: t(".reported") }
      end
    else
      errors = @bug_report.errors.full_messages
      respond_to do |format|
        format.json { render json: { ok: false, errors: errors }, status: :unprocessable_entity }
        format.html { redirect_back fallback_location: root_path, alert: errors.to_sentence.presence || t(".failed") }
      end
    end
  end

  private

  # The browser context rides along as a single JSON string. Parse defensively
  # and keep only the keys we expect, so a hand-crafted POST can't stuff
  # arbitrary data into the column.
  ALLOWED_METADATA_KEYS = %w[viewport screen device_pixel_ratio breakpoint referrer console_errors locale].freeze

  def captured_metadata
    raw = params[:metadata]
    parsed = raw.is_a?(String) ? JSON.parse(raw) : raw
    return {} unless parsed.is_a?(Hash)

    parsed.slice(*ALLOWED_METADATA_KEYS)
  rescue JSON::ParserError
    {}
  end

  def attach_screenshot
    file = params[:screenshot]
    return if file.blank?
    return unless file.respond_to?(:content_type) && file.content_type.to_s.start_with?("image/")

    @bug_report.screenshot.attach(file)
  end
end
