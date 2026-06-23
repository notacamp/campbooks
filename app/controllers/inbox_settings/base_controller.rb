module InboxSettings
  # Base for every inbox-settings panel. Panels render into the modal's
  # `inbox_settings_panel` Turbo Frame, so they never use the app layout.
  class BaseController < ::Settings::BaseController
    layout false
  end
end
