# frozen_string_literal: true

module Api
  module App
    module Settings
      module Integrations
        # Calendar connections overview (status, accounts, primary).
        class CalendarsController < Api::App::BaseController
          # GET /api/app/settings/integrations/calendars
          def show
            accounts = current_user.calendar_accounts.includes(:calendars).order(:created_at)
            render_data({
              accounts: accounts.map { |a| account_data(a) }
            })
          end

          private

          def account_data(account)
            {
              id: account.id,
              provider: account.provider,
              email: account.try(:email_address) || account.try(:email),
              active: account.active?,
              calendar_count: account.calendars.count,
              calendars: account.calendars.map { |c| calendar_data(c) }
            }
          end

          def calendar_data(calendar)
            {
              id: calendar.id,
              name: calendar.name,
              color: calendar.try(:color),
              primary: calendar.try(:primary?),
              syncing: calendar.syncing
            }
          end
        end
      end
    end
  end
end
