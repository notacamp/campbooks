class User < ApplicationRecord
  belongs_to :workspace

  has_secure_password

  # TOTP shared secret for the authenticator-app second factor. Encrypted at rest
  # via ActiveRecord::Encryption (same keys/pattern as EmailAccount#refresh_token).
  encrypts :totp_secret

  # One-time, short-lived token that hands a completed native OAuth sign-in from
  # the system auth session back into the app's web view: the native shell
  # redeems it at SessionsController#native to start a real cookie session.
  generates_token_for :native_session, expires_in: 15.minutes

  has_many :sessions, dependent: :destroy
  has_many :reviewed_documents, class_name: "Document", foreign_key: :reviewed_by_id
  has_many :created_tasks, class_name: "Task", foreign_key: :created_by_id, dependent: :nullify
  has_many :task_assignments, dependent: :destroy
  has_many :assigned_tasks, through: :task_assignments, source: :task
  has_many :notifications, dependent: :destroy
  has_many :notification_preferences, dependent: :destroy
  has_many :devices, dependent: :destroy
  has_many :feed_items, dependent: :delete_all
  has_many :bug_reports, dependent: :destroy
  # Unsent composer drafts (Dock/Desk autosave). Private to their author.
  has_many :draft_emails, dependent: :destroy
  has_many :scheduled_digests, dependent: :destroy
  has_many :digest_issues, dependent: :destroy
  # Security/audit trail. The FK is on_delete: :nullify so rows survive a user's
  # deletion (anonymized) for accountability; nullify here matches that intent.
  has_many :audit_events, dependent: :nullify
  # Async GDPR data-portability archives the user requested (purged with them).
  has_many :account_exports, dependent: :destroy
  # Images uploaded into the compose / signature editor. Attaching them to the
  # user keeps the blobs from being purged and ties them to the owner (GDPR).
  has_many_attached :outbound_images
  # Files attached to outbound emails from the composer (resolved + sent at send time).
  has_many_attached :outbound_attachments
  has_many :agent_threads, dependent: :destroy
  has_many :agent_messages, dependent: :destroy
  has_many :thread_follows, dependent: :destroy
  has_many :followed_threads, through: :thread_follows, source: :agent_thread
  has_many :email_account_users, dependent: :destroy
  has_many :email_accounts, through: :email_account_users
  has_many :readable_email_accounts, -> { merge(EmailAccountUser.where(can_read: true)) }, through: :email_account_users, source: :email_account
  has_many :sendable_email_accounts, -> { merge(EmailAccountUser.where(can_send: true)) }, through: :email_account_users, source: :email_account
  has_many :calendar_account_users, dependent: :destroy
  has_many :calendar_accounts, through: :calendar_account_users
  has_many :readable_calendar_accounts, -> { merge(CalendarAccountUser.where(can_read: true)) }, through: :calendar_account_users, source: :calendar_account
  has_many :writable_calendar_accounts, -> { merge(CalendarAccountUser.where(can_write: true)) }, through: :calendar_account_users, source: :calendar_account
  has_many :manageable_calendar_accounts, -> { merge(CalendarAccountUser.where(can_manage: true)) }, through: :calendar_account_users, source: :calendar_account
  has_many :signatures, dependent: :destroy
  has_many :invited_invitations, class_name: "Invitation", foreign_key: :invited_by_id, dependent: :nullify
  has_many :accepted_invitations, class_name: "Invitation", foreign_key: :accepted_by_id, dependent: :nullify

  # OAuth sign-in methods ("Sign in with Google/Microsoft/Zoho"). A login
  # credential, distinct from a connected mailbox — see Identity / Auth::OauthSignIn.
  has_many :identities, dependent: :destroy

  # Two-factor authentication (opt-in second factors)
  has_many :webauthn_credentials, dependent: :destroy
  has_many :recovery_codes, dependent: :destroy
  has_many :mfa_email_challenges, dependent: :destroy

  attribute :role, :integer
  enum :role, { member: 0, admin: 1 }
  # Where a brand-new email opens: the Desk (full page) or the Dock (sheet).
  enum :compose_default, { desk: 0, dock: 1 }, prefix: :composes_in

  normalizes :email_address, with: ->(e) { e.strip.downcase }

  validates :email_address, presence: true, uniqueness: true
  validates :name, presence: true
  validates :locale, inclusion: { in: ->(_user) { I18n.available_locales.map(&:to_s) } }, allow_blank: true
  # Single source of truth for password strength across every path (registration,
  # settings change, reset). has_secure_password only checks presence/confirmation;
  # allow_nil so updates that don't change the password skip the length check.
  validates :password, length: { minimum: 8 }, allow_nil: true

  def admin?
    role == "admin"
  end

  def unread_notifications_count
    notifications.badge_visible.count
  end

  # True while any account this user can read has a scan genuinely in flight.
  # Backs the live sync pill (stale "scanning" flags are excluded, see
  # EmailAccount.actively_scanning).
  def email_syncing?
    readable_email_accounts.actively_scanning.exists?
  end

  # True while any calendar account this user can read has a sync in flight.
  def calendar_syncing?
    readable_calendar_accounts.actively_scanning.exists?
  end

  # ── Primary-nav attention dots (deprecated) ─────────────────────────────────
  # Formerly tracked per-section "last seen at" timestamps for
  # Navigation::Attention. Replaced by resource-state queries (2026-06-27).
  # The section_seen_at column stays for now — harmless, allows instant
  # rollback. Will be removed in a future release.
  SECTION_KEYS = %i[home mail calendar documents scout].freeze

  # @deprecated No longer used by Navigation::Attention. Will be removed.
  def seen_section_at(section)
    raw = section_seen_at&.dig(section.to_s)
    raw ? Time.zone.parse(raw) : created_at
  end

  # @deprecated No longer used by Navigation::Attention. Will be removed.
  def mark_section_seen!(section, at: Time.current)
    return unless SECTION_KEYS.include?(section.to_sym)

    merged = (section_seen_at || {}).merge(section.to_s => at.utc.iso8601)
    update_column(:section_seen_at, merged)
  end

  def workspace_context
    workspace&.workspace_context
  end

  # ── Personal voice for Scout's reply drafts ─────────────────────────────────
  # Combines the user's manually stated writing style with the profile Scout
  # auto-learns from their sent mail. The stated text comes last so it overrides
  # / augments the learned profile. Returns "" when the user has set neither, so
  # callers can interpolate it unconditionally.
  def writing_style?
    writing_style.present? || writing_style_learned.present?
  end

  def writing_style_prompt
    return "" unless writing_style?

    parts = []
    parts << "Learned from past sent emails:\n#{writing_style_learned.strip}" if writing_style_learned.present?
    parts << "Stated preferences (these take priority):\n#{writing_style.strip}" if writing_style.present?

    <<~STYLE.strip
      ## How #{name.presence || "the user"} writes
      Match this person's voice — greeting, sign-off, formality, sentence length, and recurring phrases. Never let style override accuracy or the instructions above.
      #{parts.join("\n\n")}
    STYLE
  end

  # ── First-run guidance (one-time tours) ─────────────────────────────────────
  # Keys of guided overlays the user has already been shown, stored in the
  # dismissed_tours jsonb array (e.g. "skim_intro", "doc_skim_intro"). Mirrors
  # the section-seen pattern: a single cheap column write, no validations or
  # callbacks. Dismissed by ToursController; read by the skim views to decide
  # whether to greet the user with Campbooks::SkimIntro.
  def tour_dismissed?(key)
    Array(dismissed_tours).include?(key.to_s)
  end

  def dismiss_tour!(key)
    key = key.to_s
    return if tour_dismissed?(key)

    update_column(:dismissed_tours, (Array(dismissed_tours) + [ key ]).uniq)
  end

  # ── Per-user calendar visibility (sidebar show/hide) ────────────────────────
  # Calendars this user has hidden from their /calendar view, stored in the
  # hidden_calendar_ids jsonb array of uuid strings. Display-only and personal:
  # the calendar keeps syncing for everyone (Calendar#syncing is account-wide);
  # hiding just drops its events from this user's grid. Mirrors dismissed_tours:
  # a single cheap column write, no validations or callbacks.
  def calendar_hidden?(calendar)
    Array(hidden_calendar_ids).include?(calendar.id.to_s)
  end

  def set_calendar_hidden!(calendar, hidden)
    ids = Array(hidden_calendar_ids).map(&:to_s)
    updated = hidden ? (ids + [ calendar.id.to_s ]).uniq : ids - [ calendar.id.to_s ]
    update_column(:hidden_calendar_ids, updated) unless updated == ids
  end

  # ── Inbox smart groups ──────────────────────────────────────────────────────
  # Per-user prefs for bundling low-priority mail into collapsed inbox group
  # rows, stored in the inbox_smart_groups jsonb ({"enabled" => bool,
  # "<bucket>" => bool}). A missing key means enabled, so the feature is ON by
  # default with all buckets — the column only records opt-outs.
  SMART_GROUP_BUCKETS = %w[notifications promotions social updates].freeze

  def smart_groups_enabled?
    inbox_smart_groups.fetch("enabled", true)
  end

  def smart_group_enabled?(bucket)
    smart_groups_enabled? && inbox_smart_groups.fetch(bucket.to_s, true)
  end

  def enabled_smart_group_buckets
    return [] unless smart_groups_enabled?

    SMART_GROUP_BUCKETS.select { |bucket| inbox_smart_groups.fetch(bucket, true) }
  end

  def update_smart_group_prefs!(prefs)
    merged = inbox_smart_groups.merge(prefs.slice("enabled", *SMART_GROUP_BUCKETS))
    update!(inbox_smart_groups: merged)
  end

  # ── Two-factor authentication ───────────────────────────────────────────────
  # True when the user has any second factor turned on. Gates the login challenge
  # (SessionsController#create): password-only login when false, second-factor
  # step when true. OAuth/native sign-in bypasses the challenge regardless.
  def mfa_enabled?
    totp_enabled_at? || email_otp_enabled_at? || webauthn_credentials.exists?
  end

  # Active second factors, in the order they're offered on the challenge screen.
  def mfa_methods
    methods = []
    methods << :totp      if totp_enabled_at?
    methods << :passkey   if webauthn_credentials.exists?
    methods << :email_otp if email_otp_enabled_at?
    methods
  end

  # The stable WebAuthn user handle, generated once on first passkey enrollment.
  # Kept distinct from the DB id so it can't be correlated across relying parties.
  def ensure_webauthn_id!
    return webauthn_id if webauthn_id.present?

    update!(webauthn_id: WebAuthn.generate_user_id)
    webauthn_id
  end

  # ── Sign-in methods ─────────────────────────────────────────────────────────
  # The ways this user can actually get in: a real (user-chosen) password, a
  # passkey, or a linked OAuth identity. A synthetic OAuth-only password
  # (password_set_by_user == false) does NOT count. Used to forbid removing the
  # last remaining method (Settings → Security) and to reason about lockout.
  def sign_in_methods_count
    count = identities.count + webauthn_credentials.count
    count += 1 if password_set_by_user?
    count
  end

  def any_sign_in_method?
    sign_in_methods_count.positive?
  end
end
