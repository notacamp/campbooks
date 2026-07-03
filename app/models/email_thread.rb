class EmailThread < ApplicationRecord
  belongs_to :email_account
  has_many :email_messages, dependent: :nullify
  has_one :agent_thread, as: :contextable, dependent: :destroy
  has_many :agent_messages, through: :agent_thread
  has_many :documents, through: :email_messages

  # Safety net so every thread — however it was created — carries the normalized
  # match key that Emails::Threading groups on. Only fills a blank key; never
  # rewrites an existing one (that would re-split live conversations).
  before_save :ensure_subject_key

  def ensure_subject_key
    self.subject_key = Emails::SubjectNormalizer.key(subject.to_s) if subject_key.blank?
  end

  # A teammate can see this thread's discussion if they can read the mailbox it
  # arrived in, OR they were pulled into the discussion by an @mention (i.e. they
  # follow its agent_thread). See ThreadFollow and EmailThreadsController#show.
  def accessible_by?(user)
    return true if email_account&.accessible_by?(user)

    agent_thread.present? && ThreadFollow.exists?(user: user, agent_thread: agent_thread)
  end

  def latest_message
    if email_messages.loaded?
      email_messages.max_by { |m| m.received_at || Time.at(0) }
    else
      email_messages.order(received_at: :desc).first
    end
  end

  def participants
    email_messages.pluck(:from_address).uniq
  end

  # Distinct senders for the participant facepile, newest first, as
  # [{ email:, contact_id: }]. Deduped case-insensitively, capped. Uses already
  # loaded messages when present (the inbox row/board preload them) so it adds no
  # query there; otherwise a single pluck.
  def participant_senders(limit: 6)
    rows =
      if email_messages.loaded?
        email_messages
          .select { |m| m.from_address.present? }
          .sort_by { |m| m.received_at || Time.at(0) }.reverse
          .map { |m| [ m.from_address, m.contact_id ] }
      else
        email_messages
          .where.not(from_address: [ nil, "" ])
          .order(received_at: :desc)
          .pluck(:from_address, :contact_id)
      end

    rows.uniq { |addr, _| addr.to_s.downcase }
        .first(limit)
        .map { |addr, cid| { email: addr, contact_id: cid } }
  end

  def display_subject
    latest_message&.subject || subject
  end

  scope :expired_snoozes, -> { where("snoozed_until <= ?", Time.current) }
  scope :snoozed, -> { where("snoozed_until > ?", Time.current) }

  # A thread is pinned ("Priority") when any of its messages is pinned. The
  # not-null thread-id filter keeps this safe to negate with `where.not(id: …)`
  # — a NULL inside a NOT IN set would otherwise match no rows at all.
  scope :pinned, -> { where(id: EmailMessage.pinned.where.not(email_thread_id: nil).select(:email_thread_id)) }

  def pinned?
    if email_messages.loaded?
      email_messages.any? { |m| m.pinned_at.present? }
    else
      email_messages.where.not(pinned_at: nil).exists?
    end
  end

  def unread?
    if email_messages.loaded?
      email_messages.any? { |m| !m.read? }
    else
      email_messages.where(read: false).exists?
    end
  end

  def snoozed?
    snoozed_until.present? && snoozed_until > Time.current
  end

  # --- Reply tracking (denormalized; maintained by EmailProcessJob + send paths) ---

  # The owner had the last say in this thread — they replied and the other party
  # hasn't come back yet, so the ball is in the other party's court. A thread with
  # only outbound mail (a cold send) also counts. Used to hide answered mail from
  # Skim/the Feed and to gate follow-up surfacing.
  def holds_last_word?
    last_outbound_at.present? && (last_inbound_at.nil? || last_outbound_at >= last_inbound_at)
  end

  # The SQL counterpart of #holds_last_word? — the owner sent last and the other
  # party hasn't replied since. Pure column check, no AI. `Time.at(0)` stands in
  # for "never replied" so a thread with only outbound mail still qualifies.
  scope :holds_last_word, -> {
    where.not(last_outbound_at: nil)
         .where("last_outbound_at >= COALESCE(last_inbound_at, ?)", Time.at(0))
  }

  # An AI-confirmed follow-up that has come due and the owner hasn't dismissed.
  def follow_up_due?(now = Time.current)
    follow_up_expected? && follow_up_dismissed_at.nil? &&
      follow_up_at.present? && follow_up_at <= now
  end

  # Threads with a pending, not-dismissed follow-up that has come due (hits the
  # partial index index_email_threads_on_due_follow_ups).
  scope :follow_up_due, ->(now = Time.current) {
    where(follow_up_expected: true, follow_up_dismissed_at: nil)
      .where(follow_up_at: ..now)
  }

  # Grace window so a reply the owner sent moments ago isn't surfaced as "waiting"
  # before there's been any realistic chance of a response.
  AWAITING_REPLY_GRACE = 6.hours

  # Threads the owner is waiting on a reply for: they hold the last word, haven't
  # dismissed the nudge, and sent past the grace window. Powers every "Waiting on
  # replies" surface (inbox section, Scout briefing, feed, Skim) — see
  # Emails::AwaitingReply.
  #
  # AI as vetter, never gatekeeper: once Ai::FollowUpAnalyzer has judged the reply
  # (follow_up_last_analyzed_at set), a "no follow-up expected" verdict drops the
  # thread — an FYI / closing / acknowledgement the other party isn't expected to
  # answer isn't really "waiting". Threads it hasn't judged yet — or that it never
  # will, because no AI provider is configured — fall through on the pure-data
  # signal, so these surfaces keep working (just unvetted) without AI.
  scope :awaiting_reply, ->(cutoff = AWAITING_REPLY_GRACE.ago) {
    holds_last_word
      .where(follow_up_dismissed_at: nil)
      .where("last_outbound_at <= ?", cutoff)
      .where("follow_up_last_analyzed_at IS NULL OR follow_up_expected")
  }
end
