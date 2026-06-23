module Auth
  # Links an OAuth identity (provider + uid) to an ALREADY-AUTHENTICATED user —
  # the explicit, owner-driven counterpart to Auth::OauthSignIn. Linking lives
  # ONLY here (never in the sign-in resolver): you can attach a provider to your
  # account only from inside your own session, which is what keeps "control of a
  # connected mailbox" from ever becoming "control of the owner's account".
  class IdentityLinker
    Result = Struct.new(:status, :identity, :reason, keyword_init: true) do
      def ok? = status == :linked || status == :already_linked
    end

    def self.call(**kwargs) = new(**kwargs).call

    def initialize(user:, provider:, uid:, email: nil)
      @user     = user
      @provider = provider.to_s
      @uid      = uid.to_s.presence
      @email    = email.to_s.strip.downcase.presence
    end

    def call
      return block(:invalid) if @uid.blank?

      if (existing = Identity.find_by(provider: @provider, uid: @uid))
        return block(:linked_to_other_user) if existing.user_id != @user.id

        existing.update!(email: @email) if @email && existing.email != @email
        return Result.new(status: :already_linked, identity: existing)
      end

      Result.new(status: :linked, identity: @user.identities.create!(provider: @provider, uid: @uid, email: @email))
    rescue ActiveRecord::RecordNotUnique
      # Lost a race to create the same (provider, uid); re-resolve the winner.
      existing = Identity.find_by(provider: @provider, uid: @uid)
      return block(:linked_to_other_user) if existing && existing.user_id != @user.id

      Result.new(status: :already_linked, identity: existing)
    end

    private

    def block(reason) = Result.new(status: :blocked, reason: reason)
  end
end
