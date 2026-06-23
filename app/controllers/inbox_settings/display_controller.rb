module InboxSettings
  # Display preferences panel. These settings are client-side only (localStorage,
  # applied live by the inbox-settings-modal Stimulus controller); the server
  # just renders the controls and the list of accounts to toggle.
  class DisplayController < BaseController
    def show
      @accounts = Current.user.readable_email_accounts
    end
  end
end
