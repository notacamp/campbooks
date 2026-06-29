module InboxSettings
  # Display preferences panel. Client-side preferences (view mode, density, which
  # accounts/sections to show) live in localStorage. Provider system/noise labels
  # are no longer a blunt global toggle here — they're reviewed per-label in
  # Settings → Tags (the "Hidden labels" section).
  class DisplayController < BaseController
    def show
      @accounts = Current.user.readable_email_accounts
    end
  end
end
