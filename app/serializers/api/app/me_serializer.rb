# frozen_string_literal: true

module Api
  module App
    # The /api/app/me bootstrap payload. Deliberately lean: identity + workspace +
    # the deploy-level Features flags the SPA needs for capability gates. The
    # per-workspace billing entitlements (plan/limits) are added by the settings
    # build agent, which owns the Entitlements surface — see api-migration/settings.md.
    class MeSerializer
      def initialize(user, workspace)
        @user = user
        @workspace = workspace
      end

      def as_json(*)
        {
          user: {
            id: @user.id,
            name: @user.name,
            email: @user.email_address,
            role: @user.role,
            locale: @user.locale.presence || I18n.default_locale.to_s,
            time_zone: @user.effective_time_zone.tzinfo.name
          },
          workspace: {
            id: @workspace.id,
            name: @workspace.name,
            app_name: @workspace.app_name,
            self_hosted: Rails.application.config.self_hosted
          },
          features: feature_flags
        }
      end

      private

      # Deploy-level readiness gates (Features), safe to read anywhere. Mirrors the
      # `Features.<name>?` set the web reads.
      def feature_flags
        %i[workflows email_board document_templates email_templates microsoft tasks digests accounting imap]
          .index_with { |flag| Features.public_send("#{flag}?") }
      end
    end
  end
end
