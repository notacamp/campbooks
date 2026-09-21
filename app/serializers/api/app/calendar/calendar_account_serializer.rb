# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Serializes a CalendarAccount with its nested calendars for the accounts
      # sidebar panel of the /api/app/calendar screen payload.
      class CalendarAccountSerializer
        def initialize(account, user:, managed_ids: [])
          @account = account
          @user = user
          @managed_ids = managed_ids
        end

        def as_json
          {
            id: @account.id,
            display_name: @account.display_name,
            email_address: @account.email_address,
            color: @account.color,
            provider: @account.provider,
            can_manage: @managed_ids.include?(@account.id),
            can_write: @account.writable_by?(@user),
            is_owner: @account.owned_by?(@user),
            calendars: calendars_data
          }
        end

        private

        def calendars_data
          @account.calendars.map do |cal|
            {
              id: cal.id,
              name: cal.name,
              color: cal.display_color,
              syncing: cal.syncing,
              is_primary: cal.is_primary,
              is_writable: cal.is_writable,
              hidden: hidden_for_user?(cal)
            }
          end
        end

        def hidden_for_user?(calendar)
          Array(@user.hidden_calendar_ids).include?(calendar.id.to_s)
        end
      end
    end
  end
end
