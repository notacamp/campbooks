class EmailMessage < ApplicationRecord
  include Searchable

  belongs_to :email_account
  belongs_to :email_scan_log, optional: true
  belongs_to :email_thread, optional: true

  belongs_to :ai_analysis_message, class_name: "AgentMessage", optional: true

  has_many_attached :files
  has_many :agent_messages, through: :email_thread
  has_many :email_message_tags, dependent: :destroy
  has_many :tags, through: :email_message_tags
  has_many :document_email_messages, dependent: :destroy
  has_many :documents, through: :document_email_messages

  belongs_to :contact, optional: true

  enum :status, {
    fetched: 0,
    processing: 1,
    processed: 2,
    ignored: 3,
    failed: 4
  }

  enum :ai_priority, {
    low: 0,
    medium: 1,
    high: 2
  }, default: :medium

  scope :with_ai_todos, -> {
    where.not(ai_action_prompt: [ nil, "" ])
      .where(ai_todo_dismissed: false)
      .order(received_at: :desc)
  }

  # Permission gate: only messages on accounts the user is allowed to read.
  # The single source of truth for "which emails may this user see" — used by
  # every non-interactive entry point (Scout tools, bulk actions) so they match
  # the web controllers' Current.user.readable_email_accounts scoping.
  # Fails closed: a nil user (no established identity) sees nothing.
  scope :accessible_to, ->(user) { user ? where(email_account: user.readable_email_accounts) : none }

  # Pinned ("Priority"): mail the user promoted in Skim ("Make priority") or via
  # the inbox star. Drives the inbox Priority section and the Skim Priority ring.
  scope :pinned, -> { where.not(pinned_at: nil) }

  # Messages whose thread the owner has NOT already had the last word in — i.e. the
  # ball is still in the owner's court (or back in it after the other party replied).
  # Threadless mail always qualifies. Lets the Feed stop re-surfacing conversations
  # the owner already answered, off the denormalized email_threads watermarks
  # (last_outbound_at / last_inbound_at — see EmailThread#holds_last_word?), no N+1.
  scope :not_answered_by_owner, -> {
    where(<<~SQL.squish)
      NOT EXISTS (
        SELECT 1 FROM email_threads t
        WHERE t.id = email_messages.email_thread_id
          AND t.last_outbound_at IS NOT NULL
          AND (t.last_inbound_at IS NULL OR t.last_outbound_at >= t.last_inbound_at)
      )
    SQL
  }

  scope :by_organization, ->(org, active_only: true) {
    people_ids = Person.joins(:organization_memberships)
      .where(organization_memberships: { organization_id: org.id })
    people_ids = people_ids.where(organization_memberships: { status: :active }) if active_only
    where(contact_id: Contact.where(person_id: people_ids.select(:id)).select(:id))
  }

  validates :provider_message_id, presence: true, uniqueness: { scope: :email_account_id }

  # Keep the search index's filter metadata fresh when a filter-only field changes,
  # without paying for a re-embed (see #refresh_search_filter_data).
  after_save_commit :refresh_search_filter_data, if: :search_filter_fields_changed?

  # Human-readable thread subject (display): the subject with its leading
  # reply/forward/[tag] run stripped, original case kept. Stored on EmailThread.
  def thread_subject
    Emails::SubjectNormalizer.display(subject)
  end

  # Canonical match key used to group messages into one EmailThread (case-folded,
  # whitespace-collapsed). Distinct from #thread_subject so a noisy "RE: FW:" or
  # "Test"/"test" variant still lands in the same conversation. See
  # Emails::SubjectNormalizer for why subject-only threading needs this.
  def thread_subject_key
    Emails::SubjectNormalizer.key(subject)
  end

  def ai_processed?
    documents.exists?
  end

  def document_count
    documents.count
  end

  def tag_names
    tags.pluck(:name)
  end

  # --- Searchable concern implementation ---

  def searchable_workspace
    email_account&.workspace
  end

  def build_search_chunks
    EmailChunker.new(self).chunk.map do |c|
      c.merge(token_count: estimate_tokens(c[:content]))
    end
  end

  def searchable_title
    subject
  end

  def searchable_content_preview
    ai_summary.presence || body.to_s.truncate(500)
  end

  def searchable_filter_data
    {
      email_account_id: email_account_id,
      contact_id: contact_id,
      received_at: received_at&.iso8601,
      from_address: from_address,
      to_address: to_address,
      status: status,
      ai_priority: ai_priority,
      # Denormalized boolean column, not files.attached? — it reflects the
      # provider's metadata and avoids an Active Storage query per message.
      has_attachments: has_attachment,
      provider_folder_id: provider_folder_id,
      read: read,
      category: category,
      sender_domain: sender_domain
    }.compact
  end

  def searchable_tags
    tag_names
  end

  def searchable_source_created_at
    received_at || created_at
  end

  # Only fields that feed the embedded text trigger a full re-index (re-chunk +
  # re-embed). Filter-only fields are handled by #refresh_search_filter_data.
  def searchable_fields_changed?
    saved_change_to_subject? || saved_change_to_body? || saved_change_to_ai_summary?
  end

  # Folder/read/category/attachment don't change the text we embed, so a full
  # reindex would be wasteful. When they change, refresh just the SearchRecord's
  # filter_data + tags in place (no re-embed) so meaning-mode pre-filtering stays
  # accurate. The `&&` short-circuits, keeping the search_record lookup off the
  # common save path. (Bulk read-marking uses update_all, which skips callbacks —
  # that staleness is corrected by the SQL safety net in Emails::Search.)
  def search_filter_fields_changed?
    (saved_change_to_provider_folder_id? || saved_change_to_read? ||
      saved_change_to_category? || saved_change_to_has_attachment?) &&
      search_record.present?
  end

  def refresh_search_filter_data
    search_record&.update_columns(
      filter_data: searchable_filter_data,
      tags: searchable_tags,
      source_updated_at: Time.current
    )
  end

  def sent?
    from_address.to_s.downcase == email_account&.email_address.to_s.downcase
  end

  def external_tags
    tags.external
  end

  def document_types
    docs = if documents.loaded?
             documents
    else
             documents.includes(:classification)
    end
    docs.map(&:classification).compact.uniq
  end

  private

  # The domain part of the sender address. Handles both a bare "user@host" and the
  # RFC 5322 "Name <user@host>" form (captures up to the closing ">" or whitespace).
  def sender_domain
    from_address.to_s[/@([^>\s]+)/, 1]&.downcase.presence
  end

  def estimate_tokens(text)
    return 0 if text.blank?
    (text.length / 3.5).ceil
  end
end
