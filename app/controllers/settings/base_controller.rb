class Settings::BaseController < ApplicationController
  before_action :require_authentication

  # Settings pages live inside the settings overlay (Campbooks::SettingsOverlay).
  # A full request renders the application shell with the overlay open on this
  # page; a Turbo-Frame request for the overlay's frame renders only that frame
  # (layouts/settings_frame) so the pane swaps in place; any other frame request
  # keeps turbo-rails' default frame layout.
  layout :settings_layout

  private

  def settings_layout
    return "application" unless turbo_frame_request?

    request.headers["Turbo-Frame"] == "settings_panel" ? "settings_frame" : "turbo_rails/frame"
  end

  def current_section
    controller_name
  end
  helper_method :current_section
end
