# frozen_string_literal: true

# Stops an action that needs an AI provider the workspace hasn't configured, and
# tells the user how to switch it on.
#
# The rich, inline "set up AI" prompt (Campbooks::AiSetupPrompt) is shown
# proactively by the views, so the user normally never reaches a doomed submit.
# This is the safety net for a direct POST (a stale page, an API client) so the
# request never reaches a background job or the model. Use it as a guard clause:
#
#   def create
#     return if require_ai_provider!(:text)
#     # …enqueue the reply job…
#   end
module AiProviderGuard
  extend ActiveSupport::Concern

  private

  # Renders a "set up AI" response and returns true when <capability> can't run
  # for the current workspace; returns false (and does nothing) when it can.
  def require_ai_provider!(capability)
    return false if ai_provider_available?(capability)

    message = t("components.ai_setup_prompt.#{capability}.title")

    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(message, severity: :warning) }
      format.json { render json: { error: "ai_provider_unconfigured", capability: capability.to_s }, status: :service_unavailable }
      format.any { redirect_back fallback_location: root_path, warning: message }
    end
    true
  end
end
