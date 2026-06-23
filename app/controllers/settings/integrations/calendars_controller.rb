class Settings::Integrations::CalendarsController < Settings::BaseController
  def show
    @calendar_accounts = Current.user.calendar_accounts
                                .includes(:calendars)
                                .order(:created_at)
  end

  private

  def current_section
    "integrations"
  end
end
