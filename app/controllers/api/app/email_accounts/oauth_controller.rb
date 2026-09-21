# frozen_string_literal: true

module Api
  module App
    module EmailAccounts
      # Returns the provider OAuth authorize URL so the SPA can open it in the
      # browser tab (window.location.href = authorize_url). The browser follows
      # the consent redirect and the provider posts back to the existing callback
      # routes (/oauth/{zoho,gmail,microsoft}/callback). No new redirect URIs.
      #
      # The `state` embeds flow=account_link + spa: true + user_id + return_to so
      # OauthNativeHandoff's callback bridge can:
      #   1. Identify the user without a session cookie (spa_account_link_flow?)
      #   2. Redirect back to the SPA return_to on success/failure
      #
      # Security: return_to is validated against APP_FRONTEND_URL at mint time;
      # the full state is HMAC-signed (Oauth::State) with a 30-min TTL so a
      # captured/tampered state cannot be replayed.
      class OauthController < BaseController
        # GET /api/app/oauth/authorize_url?provider=google|zoho|microsoft&return_to=...
        def authorize_url
          provider = params[:provider].to_s

          unless provider.in?(%w[google zoho microsoft])
            return render_error("invalid_provider",
                                "Unknown provider '#{provider}'. Supported: google, zoho, microsoft.",
                                status: :unprocessable_entity)
          end

          if provider == "microsoft" && !::Features.microsoft?
            return render_error("feature_disabled",
                                "Microsoft mailbox connect is not enabled.",
                                status: :not_found)
          end

          return_to = validated_return_to(params[:return_to])

          state = ::Oauth::State.encode(
            flow: "account_link",
            user_id: current_user.id,
            spa: true,
            return_to: return_to
          )

          url = case provider
          when "google"
            ::Google::OauthClient.authorize_url(
              redirect_uri: oauth_gmail_callback_url,
              state: state,
              scopes: ::Google::OauthClient::CONNECT_SCOPES
            )
          when "zoho"
            zoho_params = {
              client_id: ENV.fetch("ZOHO_CLIENT_ID", ""),
              response_type: "code",
              redirect_uri: oauth_zoho_callback_url,
              scope: "ZohoMail.messages.ALL,ZohoMail.attachments.READ,ZohoMail.accounts.READ," \
                     "ZohoMail.folders.READ,ZohoMail.tags.ALL,ZohoCalendar.event.ALL,ZohoCalendar.calendar.ALL",
              access_type: "offline",
              prompt: "consent",
              state: state
            }
            "#{::Zoho::OauthClient::AUTH_URL}?#{zoho_params.to_query}"
          when "microsoft"
            ::Microsoft::OauthClient.authorize_url(
              redirect_uri: oauth_microsoft_callback_url,
              state: state
            )
          end

          render_data({ provider: provider, authorize_url: url, note: provider_note(provider) })
        end

        private

        # Validate and normalise return_to. Must start with APP_FRONTEND_URL (or
        # be a root-relative path) to prevent open redirects.
        def validated_return_to(raw)
          frontend_root = spa_frontend_url("/")
          candidate = raw.to_s.strip
          return spa_frontend_url("/settings/mailboxes") if candidate.blank?

          if candidate.start_with?(frontend_root) || candidate.start_with?("/")
            candidate.start_with?("/") ? spa_frontend_url(candidate) : candidate
          else
            spa_frontend_url("/settings/mailboxes")
          end
        end

        def provider_note(provider)
          case provider
          when "google"    then "Grants mail + calendar access"
          when "zoho"      then "Grants mail + calendar access"
          when "microsoft" then "Grants mail access (work/school accounts)"
          end
        end

        # Root URL of the React SPA. Mirrors OauthNativeHandoff#spa_frontend_url.
        def spa_frontend_url(path = "/")
          root = ENV.fetch("APP_FRONTEND_URL", "http://localhost:3100").chomp("/")
          path.start_with?("/") ? "#{root}#{path}" : "#{root}/#{path}"
        end
      end
    end
  end
end
