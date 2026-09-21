# frozen_string_literal: true

module Api
  module App
    module Settings
      # Current-user profile + preferences. Excludes password hashes and session
      # data. The SPA reads this on every Settings → Account open.
      class AccountSerializer
        def initialize(user)
          @user = user
        end

        def as_json(*)
          {
            id: @user.id,
            name: @user.name,
            email: @user.email_address,
            locale: @user.locale.presence || I18n.default_locale.to_s,
            time_zone: @user.time_zone,
            role: @user.role,
            compose_default: @user.compose_default,
            writing_style: @user.writing_style,
            writing_style_learned: @user.writing_style_learned,
            writing_style_updated_at: @user.writing_style_updated_at,
            deletion_requested_at: @user.deletion_requested_at,
            created_at: @user.created_at
          }
        end
      end
    end
  end
end
