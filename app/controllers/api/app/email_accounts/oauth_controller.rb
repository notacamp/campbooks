# frozen_string_literal: true

module Api
  module App
    module EmailAccounts
      # Returns the provider OAuth authorize URL so the SPA / Capacitor can open
      # it in the system browser (or an in-app browser via the Browser plugin).
      # This does NOT perform a redirect — the connect flow is always a browser
      # round-trip ending at the existing provider callback routes
      # (/oauth/{zoho,gmail,microsoft}/callback). No new redirect URIs required.
      #
      # ⚠️ IMAP accounts are connected via POST /api/app/imap_accounts — this
      # endpoint returns 422 for provider=imap.
      #
      # The `state` param is a signed Oauth::State token embedded in the URL;
      # it carries flow=account_link so the callback wires the grant to the
      # acting user's workspace.
      class OauthController < BaseController
        # GET /api/app/oauth/authorize_url?provider=zoho|google|microsoft
        def authorize_url
          provider = params[:provider].to_s

          if provider == "imap"
            return render_error("imap_redirect",
                                "IMAP accounts are connected via POST /api/app/imap_accounts.",
                                status: :unprocessable_entity)
          end

          if provider == "microsoft" && !::Features.microsoft?
            return render_not_found
          end

          state = ::Oauth::State.encode(flow: "account_link", user_id: current_user.id)
          url   = build_authorize_url(provider, state)
          return render_not_found unless url

          render_data({ provider: provider, authorize_url: url,
                        note: "Open this URL in the system browser. The callback sets up the account and the SPA receives a deep-link with the bearer token." })
        end

        private

        def build_authorize_url(provider, state)
          case provider
          when "google"
            ::Google::OauthClient.authorize_url(
              redirect_uri: oauth_gmail_callback_url,
              state: state,
              scopes: ::Google::OauthClient::CONNECT_SCOPES
            )
          when "zoho"
            auth_url = ::Zoho::OauthClient::AUTH_URL
            query = {
              client_id: ENV.fetch("ZOHO_CLIENT_ID", nil),
              response_type: "code",
              redirect_uri: oauth_zoho_callback_url,
              scope: "ZohoMail.messages.ALL,ZohoMail.attachments.READ,ZohoMail.accounts.READ," \
                     "ZohoMail.folders.READ,ZohoMail.tags.ALL,ZohoCalendar.event.ALL,ZohoCalendar.calendar.ALL",
              access_type: "offline",
              prompt: "consent",
              state: state
            }
            "#{auth_url}?#{query.to_query}"
          when "microsoft"
            ::Microsoft::OauthClient.authorize_url(
              redirect_uri: oauth_microsoft_callback_url,
              state: state
            )
          end
        end
      end
    end
  end
end
