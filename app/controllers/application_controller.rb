class ApplicationController < ActionController::Base
  include Authentication
  include Notifiable
  include Pagy::Backend
  include AiProviderGuard
  include EntitlementGuard

  allow_browser versions: :modern
  stale_when_importmap_changes

  before_action :set_current_workspace
  around_action :switch_locale
  before_action :ensure_workspace
  # Kept as a (no-op) callback because several controllers skip_before_action
  # this symbol; see the method below — onboarding is never force-redirected.
  before_action :redirect_to_onboarding_if_incomplete

  helper_method :current_user, :self_hosted?, :signup_mode, :public_signup_allowed?, :beta_code_required?,
                :workflows_enabled?, :email_board_enabled?, :microsoft_enabled?, :document_templates_enabled?, :email_templates_enabled?, :tasks_enabled?, :ai_provider_available?,
                :show_beta_banner?, :current_entitlements

  private

  def current_user
    Current.session&.user
  end

  # "Will <capability> (:text / :documents / :embeddings) actually work for the
  # current workspace right now?" Drives the inline AiSetupPrompt and the guard.
  # Memoized per request — views call it for several controls on one page.
  def ai_provider_available?(capability)
    return false unless Current.workspace

    cap = capability.to_sym
    @_ai_availability ||= {}
    return @_ai_availability[cap] if @_ai_availability.key?(cap)

    @_ai_availability[cap] = Ai::ProviderSetup.available?(Current.workspace, cap)
  end

  def self_hosted?
    Rails.application.config.self_hosted
  end

  # Effective entitlements for the current workspace, memoized per request. Views
  # gate features on this (`current_entitlements.feature?(:x)`) and controllers
  # guard actions with `require_entitlement!`. Falls back to an unlimited
  # NullResolver when there's no workspace yet (e.g. onboarding) so pre-workspace
  # flows are never blocked.
  def current_entitlements
    @_current_entitlements ||= Current.workspace&.entitlements || Entitlements::NullResolver.new
  end

  # The cloud "early beta" stripe (Campbooks::BetaBanner) shows on every page
  # until the visitor dismisses it — the beta-banner controller then drops the
  # cookie we check here, so it stops rendering on the next request. Self-hosted
  # builds never show it: those operators opted into running beta software.
  def show_beta_banner?
    !self_hosted? && cookies[:beta_banner_dismissed] != "1"
  end

  # Production-readiness feature gates (see Features). These features are built
  # but not yet production-ready, so they're hidden/inert by default and opt-in
  # via ENV. Exposed to views/components here; jobs and service objects read
  # Features.* directly.
  def workflows_enabled?
    Features.workflows?
  end

  def email_board_enabled?
    Features.email_board?
  end

  # Unlike the others, this gates EVERYTHING Microsoft, including "Sign in with
  # Microsoft" on the auth pages (the old, mailbox-only flag deliberately did
  # not). Cred presence alone isn't a safe signal — MICROSOFT_CLIENT_ID may be
  # set while the Entra app registration is still incomplete.
  def microsoft_enabled?
    Features.microsoft?
  end

  def document_templates_enabled?
    Features.document_templates?
  end

  def email_templates_enabled?
    Features.email_templates?
  end

  def tasks_enabled?
    Features.tasks?
  end

  # 404 a request for a feature gated off by a readiness flag (Features.*). Used
  # as a before_action by the controllers behind one. A 404 (rather than a
  # redirect) keeps a disabled feature from advertising its own existence.
  def require_workflows_enabled
    head :not_found unless Features.workflows?
  end

  def require_email_board_enabled
    head :not_found unless Features.email_board?
  end

  def require_document_templates_enabled
    head :not_found unless Features.document_templates?
  end

  def require_email_templates_enabled
    head :not_found unless Features.email_templates?
  end

  def require_tasks_enabled
    head :not_found unless Features.tasks?
  end

  def require_digests_enabled
    head :not_found unless Features.digests?
  end

  # ── Signup gating (see config/initializers/registration.rb) ──

  def signup_mode
    Rails.application.config.signup_mode
  end

  # Whether the public can self-register at all (false only in :invite_only mode).
  def public_signup_allowed?
    signup_mode != :invite_only
  end

  # Whether a beta invite code must be entered to create an account.
  def beta_code_required?
    signup_mode == :beta_code
  end

  def set_current_workspace
    resume_session
    Current.workspace = current_user&.workspace
  end

  def ensure_workspace
    return unless authenticated?
    return if is_a?(OnboardingController) || is_a?(RegistrationsController)
    return if is_a?(SessionsController)
    return if is_a?(InvitationsController)

    unless Current.workspace
      redirect_to onboarding_path
    end
  end

  # No-op by design (2026-06-22). This used to redirect users into the
  # onboarding wizard whenever a critical/warning SetupStatus item was
  # incomplete, which trapped people in setup. We never force onboarding now:
  # new users see the wizard once right after registration, and any remaining
  # setup is surfaced via SetupHub / banners instead. Kept defined (rather than
  # deleted) because several controllers skip_before_action this symbol.
  def redirect_to_onboarding_if_incomplete
  end

  # Resolve the request locale once and run the action (and its view render)
  # inside it. Order: ?locale= override → the signed-in user's saved preference
  # → the browser's Accept-Language → I18n.default_locale. set_current_workspace
  # has already resumed the session, so Current.user is available here.
  def switch_locale(&action)
    I18n.with_locale(resolve_locale, &action)
  end

  def resolve_locale
    explicit = params[:locale].presence || Current.user&.locale.presence
    if explicit && I18n.available_locales.map(&:to_s).include?(explicit.to_s)
      return explicit.to_sym
    end

    locale_from_accept_language || I18n.default_locale
  end

  # First Accept-Language entry whose base language we ship. The header is
  # already in client preference order (e.g. "pt-PT,pt;q=0.9,en;q=0.8").
  def locale_from_accept_language
    header = request.env["HTTP_ACCEPT_LANGUAGE"]
    return if header.blank?

    available = I18n.available_locales.map(&:to_s)
    header.split(",")
          .map { |part| part.split(";").first.to_s.strip.split("-").first&.downcase }
          .find { |lang| available.include?(lang) }&.to_sym
  end

  # Best-effort provisioning of the four default tag groups for a freshly created
  # workspace. Never blocks account creation — the category->tag bridge
  # (EmailProcessJob) self-heals any workspace still missing them.
  def provision_default_groups(workspace)
    return unless workspace

    Tags::DefaultGroups.provision!(workspace)
  rescue StandardError => e
    Rails.logger.error("[Tags::DefaultGroups] provision failed for workspace #{workspace&.id}: #{e.class}: #{e.message}")
  end
end
