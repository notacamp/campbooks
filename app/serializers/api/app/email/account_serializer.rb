# frozen_string_literal: true

module Api
  module App
    module Email
      # Serializes a connected EmailAccount for the /api/app surface. Includes
      # the acting user's permissions (read/send/manage/owner) so the SPA can
      # gate UI affordances (rename, sharing panel, disconnect) without a second
      # call. `last_scanned_at` is the column name in the schema.
      class AccountSerializer
        def initialize(account, user:)
          @account = account
          @user = user
        end

        def as_json
          eau = @account.email_account_users.find_by(user: @user)
          {
            id: @account.id,
            email_address: @account.email_address,
            display_name: @account.display_name,
            provider: @account.provider,
            active: @account.active,
            scanning: @account.scanning,
            last_scanned_at: @account.last_scanned_at&.iso8601,
            push_watch_expires_at: @account.push_watch_expires_at&.iso8601,
            can_read: eau&.can_read || false,
            can_send: eau&.can_send || false,
            can_manage: eau&.can_manage || false,
            is_owner: eau&.owner || false
          }
        end
      end
    end
  end
end
