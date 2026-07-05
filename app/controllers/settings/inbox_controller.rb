# frozen_string_literal: true

module Settings
  # Settings dashboard pages for the inbox. Each inbox-settings panel (the same
  # panels as the inbox gear-icon modal) gets its own page under the "Inbox"
  # sidebar group; the page embeds that panel via the shared `inbox_settings_panel`
  # Turbo Frame, so the existing InboxSettings::* controllers render into it
  # unchanged. The panel to show is `params[:section]`, validated against the
  # shared InboxSettings::Sections catalog.
  class InboxController < Settings::BaseController
    before_action :set_section

    def show; end

    private

    def set_section
      @section = InboxSettings::Sections::ALL.find { |section| section[:key] == params[:section] }
      head :not_found unless @section
    end

    # Highlights the matching item in the settings sidebar's "Inbox" group
    # (keys are "inbox_<section>"; see NavigationHelper#inbox_settings_nav_group).
    def current_section
      "inbox_#{params[:section]}"
    end
  end
end
