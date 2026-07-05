module Auth
  # Resolves an OAuth "Sign in with Google/Microsoft/Zoho" callback to a user —
  # the single source of truth shared by all three OAuth sign-in controllers.
  #
  # The cardinal rule: a sign-in only ever lands on a user who has an *explicit*
  # Identity for this (provider, uid). Matching by email NEVER signs anyone in —
  # otherwise anyone controlling a provider account with a victim's address (or a
  # shared mailbox) could walk into the victim's account. Email matches instead
  # BLOCK with guidance toward the authenticated "add a sign-in method" flow.
  #
  # Resolution order:
  #   A. Identity(provider, uid) exists      → SIGN IN (unless deletion pending)
  #   B. a User has this login email         → BLOCK :existing_account
  #   C. a mailbox is connected at this email → BLOCK :mailbox_has_owner/_no_owner
  #   D. nothing matches                     → CREATE user + fresh workspace + Identity
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
    def initialize(provider:, uid:, email:, name: nil)
      @provider = provider.to_s
      @uid      = uid.to_s.presence
      @email    = email.to_s.strip.downcase.presence
      @name     = name.to_s.strip.presence
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

      return block(:existing_account, :warning) if User.exists?(email_address: @email)

      if (account = connected_mailbox)
        owner = account.email_account_users.exists?(owner: true)
        return block(owner ? :mailbox_has_owner : :mailbox_no_owner, :warning)
      end

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

    # New person: their own fresh workspace (mirrors RegistrationsController) — NOT
    # a workspace grouped by email domain, which would seat unrelated strangers
    # who share a provider (e.g. two @gmail.com users) in one tenant.
    def create_account
      user = nil
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
