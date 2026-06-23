# A way to sign in AS a user — distinct from a connected mailbox (EmailAccount).
#
# "Sign in with Google/Microsoft/Zoho" resolves to a user through an Identity,
# keyed on the provider's stable account id (uid), never on email. A mailbox you
# connect is a *resource* (shareable via EmailAccountUser); an Identity is a
# *credential* and is created only by the account owner, explicitly, in Settings
# (or at first OAuth signup). Keeping the two apart is what prevents "control of a
# shared mailbox → control of the owner's account". See Auth::OauthSignIn.
class Identity < ApplicationRecord
  PROVIDERS = %w[ google microsoft zoho ].freeze

  belongs_to :user

  validates :provider, presence: true, inclusion: { in: PROVIDERS }
  validates :uid, presence: true, uniqueness: { scope: :provider }

  # Human label for flash/UI ("google" → "Google").
  def provider_label
    provider.to_s.titleize
  end
end
