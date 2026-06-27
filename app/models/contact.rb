class Contact < ApplicationRecord
  begin
    # callbacks: :async — reindex via Active Job (Solid Queue), not inline in the
    # save/commit. A Contact write (block!/star!/allow! and scan-time create!)
    # must not block on or fail with OpenSearch: if the cluster is down or slow,
    # the write still commits and the reindex job retries. Inline callbacks raise
    # "Failed to open TCP connection to ...:9200" straight out of after_commit.
    searchkick word_start: [ :name, :email, :organization, :aliases ], callbacks: :async
  rescue => e
    Rails.logger.warn("[Contact] searchkick unavailable: #{e.message}")
  end

  include Searchable

  belongs_to :workspace
  belongs_to :email_account, optional: true
  belongs_to :person, optional: true
  belongs_to :suggested_person, class_name: "Person", optional: true

  has_many :email_messages, foreign_key: :contact_id, dependent: :nullify
  has_many :tags, through: :email_messages
  has_many :contact_email_aliases, dependent: :destroy
  has_many :contact_tags, dependent: :destroy
  # Tags that characterize what this sender usually sends (AI-assigned + manual).
  # Distinct from `:tags`, which is the transitive set drawn from their messages.
  has_many :sender_tags, through: :contact_tags, source: :tag

  # Sender list state. neutral = no decision; allowed = whitelisted;
  # blocked = blacklisted (mail auto-archived). Independent of `starred_at`.
  enum :list_status, { neutral: 0, allowed: 1, blocked: 2 }

  validates :email, presence: true, uniqueness: { message: :taken }

  scope :analyzed, -> { where.not(analyzed_at: nil) }
  scope :needs_analysis, -> { where(analyzed_at: nil) }
  scope :by_last_email, -> { order(last_email_at: :desc) }
  scope :without_person, -> { where(person_id: nil) }
  scope :flagged_as_duplicate, -> { where.not(suggested_person_id: nil) }
  scope :starred, -> { where.not(starred_at: nil) }
  # "Pending" = undecided sender (neutral, not starred) — the contact-skim queue
  # and, in whitelist mode, Skim's Pending ring. Mirrors #pending?.
  scope :pending, -> { neutral.where(starred_at: nil) }

  def search_data
    {
      name: name,
      email: email,
      organization: organization,
      aliases: contact_email_aliases.pluck(:email).join(" "),
      workspace_id: workspace_id
    }
  end

  def display_name
    person&.name.presence || name.presence || email.split("@").first.tr(".", " ").titleize
  end

  def needs_analysis?
    return person.needs_analysis? if person.present?
    analyzed_at.nil? || analyzed_at < 30.days.ago
  end

  def global?
    email_account_id.nil?
  end

  def promote_to_global!
    update!(email_account_id: nil)
  end

  # --- sender list / star state ---------------------------------------------
  def starred? = starred_at.present?

  def star!
    update!(starred_at: Time.current)
    track_event("contact.starred")
  end

  def unstar!
    update!(starred_at: nil)
    track_event("contact.unstarred")
  end

  # Block also clears any star; the two are contradictory.
  def block!
    update!(list_status: :blocked, starred_at: nil)
    track_event("contact.blocked")
  end

  def unblock!
    update!(list_status: :neutral)
    track_event("contact.unblocked")
  end

  def allow! = update!(list_status: :allowed)

  # In whitelist mode, a neutral (undecided), non-starred sender is "pending":
  # their mail waits in Skim's Pending bucket for an allow/deny decision.
  def pending? = neutral? && !starred?

  def related_documents
    Document.joins(:document_email_messages)
            .where(document_email_messages: { email_message_id: email_messages.select(:id) })
            .distinct
  end

  def analysis_stale?
    analyzed_at.nil? || analyzed_at < 30.days.ago
  end

  # The contact's organization name, preferring the joined Organization model via
  # their Person, falling back to the legacy free-text `organization` column.
  def organization_name
    person&.organization_name || read_attribute(:organization)
  end

  # --- Searchable concern implementation ---

  def build_search_chunks
    content = [
      ("Name: #{name}" if name.present?),
      ("Email: #{email}"),
      ("Organization: #{organization}"),
      ("Context: #{context_summary}"),
      ("Patterns: #{communication_patterns}")
    ].compact.join("\n\n")

    [ {
      content: content.presence || email,
      chunk_type: "text",
      metadata: {
        source: "contact_analysis",
        analyzed_at: analyzed_at&.iso8601
      }
    } ]
  end

  def searchable_title
    display_name
  end

  def searchable_content_preview
    context_summary.presence || "Contact: #{email}"
  end

  def searchable_filter_data
    {
      email: email,
      organization_name: organization,
      relationship_type: relationship_type,
      email_account_id: email_account_id,
      person_id: person_id,
      last_email_at: last_email_at&.iso8601,
      analyzed: analyzed_at.present?
    }.compact
  end

  def searchable_tags
    tags.pluck(:name)
  end

  def searchable_source_created_at
    last_email_at || created_at
  end

  def searchable_fields_changed?
    saved_change_to_context_summary? || saved_change_to_communication_patterns? ||
      saved_change_to_name? || saved_change_to_email?
  end

  private

  # Records a sender-list event (Current.user is the actor; system if none).
  def track_event(event_name)
    Events.publish(event_name, subject: self, payload: { "name" => name, "email" => email })
  end
end
