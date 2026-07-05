# frozen_string_literal: true

module Settings
  # Settings dashboard home for the inbox. Renders the same panels as the
  # inbox gear-icon modal (Campbooks::InboxSettingsModal) inside the settings
  # layout: a sub-nav of sections (InboxSettings::Sections) driving the shared
  # `inbox_settings_panel` Turbo Frame, which the existing InboxSettings::*
  # controllers serve unchanged.
  #
  # `current_section` defaults to controller_name ("inbox"), which matches the
  # sidebar nav's active key — no override needed.
  class InboxController < Settings::BaseController
    def show; end
  end
end
