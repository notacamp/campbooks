class SessionsController < ApplicationController
  allow_unauthenticated_access only: %i[ new create zoho google microsoft native ]
  before_action :redirect_if_authenticated, only: %i[ new create ]
  rate_limit to: 10, within: 3.minutes, only: :create, with: -> { redirect_to new_session_path, error: t(".try_later") }
  # "Sign in with Microsoft" is gated together with everything else Microsoft
  # (Features.microsoft?). 404 the route when off so a stale/crafted link can't
  # reach the half-wired Entra flow.
  before_action -> { head :not_found unless microsoft_enabled? }, only: :microsoft

  def new
    # In the native app, link OAuth buttons straight to the provider authorize
    # URL instead of /session/:provider. Going through our own redirect makes
    # Hotwire Native both URLSession-preflight *and* WebView-navigate the link,
    # so the provider URL gets handed to the system browser twice (two in-app
    # browsers). A direct cross-domain link is a single navigation → one browser.
    @oauth_urls = native_oauth_urls if hotwire_native_app?
  end

  def create
    if user = User.authenticate_by(params.permit(:email_address, :password))
      if user.deletion_requested_at.present?
        redirect_to new_session_path, error: t(".invalid")
      elsif user.mfa_enabled?
        start_mfa_challenge_for user
      else
        start_new_session_for user
        redirect_to after_authentication_url
      end
    else
      redirect_to new_session_path, error: t(".invalid")
    end
  end

  def zoho
    redirect_to provider_authorize_url(:zoho), allow_other_host: true
  end

  def google
    redirect_to provider_authorize_url(:google), allow_other_host: true
  end

  def microsoft
    redirect_to provider_authorize_url(:microsoft), allow_other_host: true
  end

  # Native OAuth handoff: the system auth session completed sign-in on the server
  # and handed a one-time token back to the app via the campbooks:// scheme. The
  # app loads this in its main web view, so start_new_session_for lands the
  # session cookie in the web view's own cookie store (see OauthNativeHandoff).
  def native
    user = User.find_by_token_for(:native_session, params[:token])
    # Mirror the password path's deletion guard (SessionsController#create): a
    # token minted before the user requested deletion must not still sign them in.
    if user && user.deletion_requested_at.nil?
      start_new_session_for user
      redirect_to after_authentication_url
    else
      redirect_to new_session_path, error: t(".expired")
    end
  end

  def destroy
    terminate_session
    redirect_to new_session_path, status: :see_other
  end

  private

  # Provider authorize URLs for the native login screen, keyed by provider.
  # Providers whose credentials aren't configured are skipped (the view falls
  # back to the /session/:provider redirect for those).
  def native_oauth_urls
    providers = %i[ google zoho ]
    providers << :microsoft if microsoft_enabled?
    providers.index_with do |provider|
      provider_authorize_url(provider)
    rescue => e
      Rails.logger.warn("[oauth] native authorize URL for #{provider} unavailable: #{e.class}")
      nil
    end.compact
  end

  # Builds a provider's OAuth authorize URL with our signed, native-aware state.
  # Shared by the redirect actions (web) and #new (which links native buttons
  # straight to it to avoid the double-navigation the /session redirect causes).
  def provider_authorize_url(provider)
    state = Oauth::State.encode(flow: "sign_in", native: hotwire_native_app? || params[:native].present?)
    case provider
    when :google
      Google::OauthClient.authorize_url(redirect_uri: oauth_gmail_callback_url, state: state)
    when :microsoft
      Microsoft::OauthClient.authorize_url(redirect_uri: oauth_microsoft_callback_url, state: state)
    when :zoho
      zoho_params = {
        client_id: ENV.fetch("ZOHO_CLIENT_ID"),
        response_type: "code",
        redirect_uri: oauth_zoho_callback_url,
        scope: "ZohoMail.accounts.READ",
        access_type: "offline",
        prompt: "consent",
        state: state
      }
      "#{Zoho::OauthClient::AUTH_URL}?#{zoho_params.to_query}"
    end
  end
end
