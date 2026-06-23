# Per-calendar settings within a connected account (the sync on/off toggle and
# color override), nested under calendar_accounts. Distinct from CalendarController
# (singular), which renders the calendar page itself.
class CalendarsController < ApplicationController
  before_action :require_authentication

  def update
    account = Current.user.calendar_accounts.find(params[:calendar_account_id])
    calendar = account.calendars.find(params[:id])

    unless account.managed_by?(Current.user)
      return redirect_to settings_integrations_calendars_path, error: t(".not_permitted")
    end

    calendar.update(calendar_params)
    redirect_to settings_integrations_calendars_path, success: t(".updated", name: calendar.name)
  end

  private

  def calendar_params
    params.require(:calendar).permit(:syncing, :color)
  end
end
