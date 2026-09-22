# frozen_string_literal: true

module Api
  module App
    module Oauth
      # Start endpoint for "Sign in with Google/Zoho/Microsoft" from the React SPA.
      # UNAUTHENTICATED (no user yet) — the SPA login page calls it, opens the
      # returned URL in the browser, and the provider posts back to the existing
      # /oauth/{gmail,zoho,microsoft}/callback, which resolves the sign-in and
      # (for a verified spa+sign_in Oauth::State) hands a one-time token to the SPA
      # return_to. See OauthNativeHandoff#spa_sign_in_flow?. No new redirect URIs.
      #
      # Sign-in scopes are mail-only (mirrors the web SessionsController) — the
      # connect flow (mail+calendar) is a separate, authenticated endpoint.
      class SignInController < Api::App::BaseController
        # Sign-in happens before there's a session, so no bearer is required.
        skip_before_action :authenticate_session_token!, only: %i[sign_in_url providers]

        PROVIDERS = %w[google zoho microsoft].freeze

        # GET /api/app/oauth/providers — the ENABLED sign-in providers, so the
        # unauthenticated login page knows which buttons to render (it can't read
        # Features.microsoft? from the token-gated /api/app/me). Microsoft appears
        # only when Features.microsoft? — the gate stays server-side.
        def providers
          list = %w[google zoho]
          list << "microsoft" if ::Features.microsoft?
          render_data({ providers: list })
        end

        # GET /api/app/oauth/sign_in_url?provider=google|zoho|microsoft&return_to=
        def sign_in_url
          provider = params[:provider].to_s

          unless PROVIDERS.include?(provider)
            return render_error("invalid_provider",
                                "Unknown provider '#{provider}'. Supported: google, zoho, microsoft.",
                                status: :unprocessable_entity)
          end

          if provider == "microsoft" && !::Features.microsoft?
            return render_error("feature_disabled", "Microsoft sign-in is not enabled.",
                                status: :not_found)
          end

          state = ::Oauth::State.encode(
            flow: "sign_in",
            spa: true,
            return_to: validated_return_to(params[:return_to])
          )

          render_data({ provider: provider, authorize_url: authorize_url_for(provider, state) })
        end

        private

        def authorize_url_for(provider, state)
          case provider
          when "google"
            ::Google::OauthClient.authorize_url(redirect_uri: oauth_gmail_callback_url, state: state)
          when "microsoft"
            ::Microsoft::OauthClient.authorize_url(redirect_uri: oauth_microsoft_callback_url, state: state)
          when "zoho"
            zoho_params = {
              client_id: ENV.fetch("ZOHO_CLIENT_ID", ""),
              response_type: "code",
              redirect_uri: oauth_zoho_callback_url,
              scope: "ZohoMail.accounts.READ",
              access_type: "offline",
              prompt: "consent",
              state: state
            }
            "#{::Zoho::OauthClient::AUTH_URL}?#{zoho_params.to_query}"
          end
        end

        # Open-redirect guard: return_to must be under APP_FRONTEND_URL or a
        # root-relative path; anything else falls back to the SPA login. Re-checked
        # at the callback (spa_callback_url) as belt-and-suspenders.
        def validated_return_to(raw)
          root = spa_frontend_url("/")
          candidate = raw.to_s.strip
          return spa_frontend_url("/login") if candidate.blank?

          if candidate.start_with?(root) || candidate.start_with?("/")
            candidate.start_with?("/") ? spa_frontend_url(candidate) : candidate
          else
            spa_frontend_url("/login")
          end
        end

        # Prefers APP_FRONTEND_URL; when unset the SPA is served same-origin with
        # this API (prod + self-hosted, behind the reverse proxy), so derive the
        # origin from the request. Local dev runs the SPA on its own port, so fall
        # back to :3100 only when the request itself is to localhost. Mirrors
        # OauthNativeHandoff#spa_frontend_url (the callback side).
        def spa_frontend_url(path = "/")
          root = spa_frontend_root
          path.start_with?("/") ? "#{root}#{path}" : "#{root}/#{path}"
        end

        def spa_frontend_root
          configured = ENV["APP_FRONTEND_URL"].presence
          return configured.chomp("/") if configured

          base = request.base_url
          base.match?(/localhost|127\.0\.0\.1/) ? "http://localhost:3100" : base
        end
      end
    end
  end
end
