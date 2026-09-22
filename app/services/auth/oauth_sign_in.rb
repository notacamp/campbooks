module Auth
  # Resolves an OAuth "Sign in with Google/Microsoft/Zoho" callback to a user —
  # the single source of truth shared by all three OAuth sign-in controllers.
  #
  # The cardinal rule: a sign-in only ever lands on a user who has an *explicit*
  # Identity for this (provider, uid) — OR whose own login email the provider has
  # cryptographically verified this person controls. A merely *claimed* (provider-
  # unverified) email, or a shared mailbox, NEVER signs anyone in — otherwise
  # anyone asserting a victim's address could walk into their account.
  #
  # Why provider-verified email → auto-link is safe: it is strictly equivalent to
  # the email-based password reset we already trust. To get a provider token that
  # says email_verified for address X you must control X's provider account; and
  # whoever controls X can already reset the password of the account whose login
  # is X. So linking on a verified email grants nothing a determined holder of X
  # couldn't already obtain — while turning the old dead-end into "Sign in with
  # Google just works". A connected *mailbox* (Case C) is deliberately NOT enough:
  # a mailbox can be shared or reassigned, so its OAuth must not confer full
  # account access to whoever currently holds it.
  #
  # Resolution order:
  #   A. Identity(provider, uid) exists       → SIGN IN (unless deletion pending)
  #   B. a User has this login email          → LINK + SIGN IN if the provider
  #                                             verified the email, else BLOCK
  #                                             :existing_account
  #   C. a mailbox is connected at this email → BLOCK :mailbox_has_owner/_no_owner
  #   D. nothing matches                      → CREATE user + fresh workspace + Identity
  class OauthSignIn
    # Outcome of a resolution. The controller turns this into a session (sign_in)
    # or a redirect-with-flash (block); `reason` keys the i18n guidance message and
    # `severity` its flash channel (see config/locales/*/auth.yml).
    Result = Struct.new(:status, :user, :reason, :severity, keyword_init: true) do
      def signed_in? = status == :sign_in
      def blocked?   = status == :block
    end

    def self.call(**kwargs) = new(**kwargs).call

    # provider: :google/:microsoft/:zoho · uid: stable provider account id ·
    # email: address from discovery · name: display name (optional).
    #
    # allow_create: when false, an unmatched identity BLOCKS (:signup_closed)
    # instead of self-serve-creating a workspace — so a caller can make social
    # sign-in respect signup_mode (e.g. the SPA on beta_code cloud). Defaults
    # true, preserving the existing web/native "create on first sign-in" behavior.
    #
    # email_verified: whether the PROVIDER asserts it verified the user controls
    # this email (Google userinfo verified_email / OIDC email_verified). Only a
    # verified email may auto-link to an existing account (Case B); it defaults
    # false, so any caller that can't vouch for the provider keeps the old block.
    def initialize(provider:, uid:, email:, name: nil, allow_create: true, email_verified: false)
      @provider = provider.to_s
      @uid      = uid.to_s.presence
      @email    = email.to_s.strip.downcase.presence
      @name     = name.to_s.strip.presence
      @allow_create = allow_create
      @email_verified = email_verified == true
      @attempts = 0
    end

    def call
      @attempts += 1

      # A missing uid means discovery misbehaved; never fall through to an
      # email-only match (that is exactly the takeover path we're closing).
      return block(:invalid, :error) if @uid.blank? || @email.blank?

      if (identity = Identity.find_by(provider: @provider, uid: @uid))
        return resolve_identity(identity)
      end

      if (user = User.find_by(email_address: @email))
        # The account's own login email. A provider-verified email is proof of
        # control (≡ the email password-reset the app already trusts) → link this
        # identity and sign in. A merely claimed email stays a block toward the
        # authenticated "add a sign-in method" flow.
        #
        # BUT never auto-link into an account with app 2FA enabled: linking a new
        # identity would let the SPA/native one-time-token handoff (which is
        # provider-MFA only, by design) skip that 2FA on this AND every later
        # sign-in. A 2FA user must link deliberately via Settings → Security,
        # which clears 2FA first. Non-2FA accounts auto-link (nothing to bypass).
        return link_and_sign_in(user) if @email_verified && !user.mfa_enabled?

        return block(:existing_account, :warning)
      end

      if (account = connected_mailbox)
        owner = account.email_account_users.exists?(owner: true)
        return block(owner ? :mailbox_has_owner : :mailbox_no_owner, :warning)
      end

      # Signup gate: a caller that doesn't allow self-serve creation (SPA on
      # beta_code cloud) blocks an unmatched identity instead of founding a
      # workspace. Existing callers (web/native) pass allow_create: true → unchanged.
      return block(:signup_closed, :warning) unless @allow_create

      create_account
    rescue ActiveRecord::RecordNotUnique, ActiveRecord::RecordInvalid
      # Concurrent first sign-in won the race between our checks and the insert.
      # Re-resolve once: the winner now exists, so we land on A (sign in) or B
      # (block) without creating a duplicate.
      raise if @attempts >= 2
      call
    end

    private

    def resolve_identity(identity)
      user = identity.user
      return block(:deletion_requested, :error) if user.deletion_requested_at.present?

      identity.update!(email: @email) if identity.email != @email
      sign_in(user)
    end

    # Case B with a provider-verified email: attach this (provider, uid) to the
    # matching account and sign in. Next time it resolves via Case A. The unique
    # (provider, uid) index + the caller's retry make a concurrent link idempotent.
    def link_and_sign_in(user)
      return block(:deletion_requested, :error) if user.deletion_requested_at.present?

      Identity.create!(user: user, provider: @provider, uid: @uid, email: @email)
      # Security-relevant: record that a sign-in method was attached automatically
      # (mirrors the explicit Settings → Security "add sign-in method" audit event),
      # so the account owner has a forensic marker in their audit log.
      AuditEvent.log("sign_in_method_added", user: user, provider: @provider, auto_linked: true)
      sign_in(user)
    end

    # New person: their own fresh workspace (mirrors RegistrationsController) — NOT
    # a workspace grouped by email domain, which would seat unrelated strangers
    # who share a provider (e.g. two @gmail.com users) in one tenant.
    def create_account
      user = nil
      workspace = nil
      ActiveRecord::Base.transaction do
        workspace = Workspace.create!(name: workspace_name, slug: "ws-#{SecureRandom.hex(4)}")
        user = workspace.users.create!(
          email_address: @email,
          name: @name || @email.split("@").first,
          password: SecureRandom.hex(32),
          password_set_by_user: false,
          # Founding a workspace makes you its admin; the instance's very
          # first account also operates the instance (mirrors registration).
          role: :admin,
          app_admin: !User.exists?
        )
        user.identities.create!(provider: @provider, uid: @uid, email: @email)
      end
      # Mirror RegistrationsController#complete: provision the managed AI default
      # and the four default tag groups so the new workspace is ready from the
      # first sign-in. Both are best-effort (never block sign-in).
      begin; Ai::ProviderSetup.apply_managed_default(workspace); rescue StandardError; end
      begin; Tags::DefaultGroups.provision!(workspace); rescue StandardError; end
      sign_in(user)
    end

    def connected_mailbox
      EmailAccount.where("LOWER(email_address) = ?", @email).first
    end

    def workspace_name
      first = @name.to_s.split(" ").first.presence || @email.split("@").first
      "#{first}'s Workspace"
    end

    def sign_in(user) = Result.new(status: :sign_in, user: user)
    def block(reason, severity) = Result.new(status: :block, reason: reason, severity: severity)
  end
end
