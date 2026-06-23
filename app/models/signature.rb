class Signature < ApplicationRecord
  # Marker class on the wrapper so any path that already embedded a signature
  # (an AI draft, a re-opened provider draft) can be detected and not doubled.
  MARKER_CLASS = "email-signature"

  belongs_to :user

  has_many :email_account_signatures, dependent: :destroy
  has_many :email_accounts, through: :email_account_signatures

  validates :name, presence: true, uniqueness: { scope: :user_id }
  validates :content, presence: true

  before_save :sanitize_content

  scope :ordered, -> { order(is_default: :desc, name: :asc) }

  def self.default_for(user, email_account = nil)
    scope = where(user: user)
    if email_account
      # Try account-specific default first, then fall back to global default
      account_default = scope.joins(:email_account_signatures)
                             .where(email_account_signatures: { email_account_id: email_account.id })
                             .ordered.first
      return account_default if account_default
    end
    scope.ordered.first
  end

  # Append `signature` to an HTML email `body`, with visual separation and a
  # wrapper carrying the marker class + id. Idempotent: if the body already
  # carries a signature (e.g. an AI-drafted body), it is returned untouched so
  # the signature is never duplicated. This is the single place signatures get
  # attached — the composer, and the AI draft tools, all route through here.
  def self.append_to_body(body, signature)
    body = body.to_s
    return body if signature.nil?
    return body if body.include?(MARKER_CLASS)

    separator = body.strip.empty? ? "" : "<br><br>"
    "#{body}#{separator}#{signature.to_email_html}"
  end

  # The sendable HTML for this signature: its (already-sanitized) content wrapped
  # in a marker element so it reads as a distinct block and can be detected later.
  def to_email_html
    %(<div class="#{MARKER_CLASS}" data-signature-id="#{id}">#{content}</div>)
  end

  def make_default!
    user.signatures.update_all(is_default: false)
    update!(is_default: true)
  end

  private

  # The rich-text signature is rendered as HTML in compose, so strip scripts /
  # event handlers / javascript: URLs (Loofah :prune keeps normal formatting).
  def sanitize_content
    return if content.blank?

    self.content = Loofah.fragment(content.to_s).scrub!(:prune).to_html
  end
end
