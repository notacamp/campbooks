module InboxSettings
  # Display preferences panel. Client-side preferences (view mode, density, …)
  # live in localStorage; the system-labels toggle writes to workspace settings
  # so it survives across devices.
  class DisplayController < BaseController
    def show
      @accounts = Current.user.readable_email_accounts
      @show_system_labels = Current.workspace.setting("show_system_labels")
    end

    def update
      org = Current.workspace
      show = ActiveModel::Type::Boolean.new.cast(params[:show_system_labels])
      org.update(settings: org.settings.merge("show_system_labels" => show))
      redirect_to inbox_settings_display_path
    end
  end
end
