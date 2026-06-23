# frozen_string_literal: true

# Unified user-notification surface.
#
# Canonical severities: :success, :error, :warning, :info.
#   - success / info  → ephemeral toast (bottom-right, auto-dismiss)
#   - error / warning → inline alert (top of <main>)
#
# Full-page responses set flash and let the shared partials render it:
#   redirect_to path, success: "Saved"
#   redirect_to path, error:   "Couldn't save"
#   flash.now[:error] = "..."   # on re-render (e.g. validation)
#
# Turbo Stream responses build an ephemeral toast inline:
#   render turbo_stream: notify_stream("Saved")
#   render turbo_stream: [other_streams, notify_stream("Sent", severity: :success)]
module Notifiable
  extend ActiveSupport::Concern

  SEVERITIES = %i[success error warning info].freeze

  # Severities that render as ephemeral toasts on a full-page render.
  # The rest (error/warning) render as inline alerts.
  TOAST_SEVERITIES = %i[success info].freeze

  # Every recognized flash key mapped to a canonical severity. Rails' built-in
  # notice/alert are aliased (notice→success, alert→error) so any legacy or
  # library-generated flash still renders through the unified partials.
  FLASH_SEVERITY = {
    "success" => :success, "notice" => :success,
    "error"   => :error,   "alert"  => :error,
    "warning" => :warning,
    "info"    => :info
  }.freeze

  included do
    add_flash_types(*SEVERITIES)
    helper_method :flash_toasts, :flash_inline_alerts
  end

  # Flashes that render as ephemeral toasts (success/info).
  def flash_toasts
    flash_notifications.select { |n| TOAST_SEVERITIES.include?(n[:severity]) }
  end

  # Flashes that render as inline alerts at the top of the page (error/warning).
  def flash_inline_alerts
    flash_notifications.reject { |n| TOAST_SEVERITIES.include?(n[:severity]) }
  end

  # Normalizes the flash into [{ severity:, message: }], aliasing notice/alert.
  def flash_notifications
    flash.filter_map do |key, message|
      severity = FLASH_SEVERITY[key.to_s]
      next if severity.nil? || message.blank?

      { severity: severity, message: message }
    end
  end

  # Builds a single turbo_stream that appends an ephemeral toast to the toast
  # region. Replaces the per-controller toast_stream helpers.
  def notify_stream(message, severity: :success)
    turbo_stream.append(
      Campbooks::ActionToast::REGION_ID,
      partial: "shared/action_toast",
      locals: { message: message, variant: severity.to_sym }
    )
  end
end
