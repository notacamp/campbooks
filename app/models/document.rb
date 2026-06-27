class Document < ApplicationRecord
  include Searchable

  belongs_to :workspace

  has_one_attached :original_file
  has_one_attached :processed_pdf

  belongs_to :reviewed_by, class_name: "User", optional: true
  belongs_to :email_account, optional: true
  belongs_to :classification, class_name: "DocumentType", foreign_key: :document_type_id, optional: true
  # Legacy association — use email_messages (through document_email_messages) instead
  belongs_to :email_message, foreign_key: :email_message_id, primary_key: :provider_message_id, optional: true

  has_many :document_email_messages, dependent: :destroy
  has_many :email_messages, through: :document_email_messages
  has_many :email_threads, through: :email_messages

  # Stage 3 "filesystem" layer — folders this document is filed into (its *place*,
  # orthogonal to its DocumentType *kind*).
  has_many :folder_memberships, as: :folderable, dependent: :destroy
  has_many :mail_folders, through: :folder_memberships
  scope :in_folder, ->(folder_id) { joins(:folder_memberships).where(folder_memberships: { mail_folder_id: folder_id }) }

  has_one :notion_page, dependent: :destroy
  has_one :notion_database_mapping, through: :classification

  enum :document_type, {
    expense_invoice: 0,
    revenue_invoice: 1,
    bank_statement: 2,
    receipt: 3,
    other: 4,
    insurance_policy: 5,
    vehicle_document: 6,
    contract: 7,
    certificate: 8,
    tax_document: 9,
    identification: 10,
    proposal: 11,
    correspondence: 12,
    bank_journal_entry: 13
  }

  # Two orthogonal lifecycles, split out of a single overloaded `status` enum:
  # ai_status is the AI processing pipeline, review_status is the human sign-off.
  # Both share the keys `pending`/`failed`, so each is prefixed to keep the generated
  # predicates collision-free: ai_pending?/ai_processing?/ai_completed?/ai_failed?,
  # review_pending?/review_approved?/review_rejected?.
  enum :ai_status, {
    pending: 0,
    processing: 1,
    completed: 2,
    failed: 3
  }, prefix: :ai

  enum :review_status, {
    pending: 0,
    approved: 1,
    rejected: 2
  }, prefix: :review

  enum :source, {
    manual_upload: 0,
    email: 1,
    notion: 2,
    sent_email: 3
  }

  enum :google_drive_push_status, {
    not_pushed: 0,
    pushed: 1,
    failed: 2
  }, prefix: :drive

  # Expense sub-category for expense_invoice docs (nullable — not every expense is
  # categorised). Prefixed because `other` would otherwise clash with the
  # document_type `other?` predicate.
  enum :expense_category, {
    travel: 0,
    meals: 1,
    office_supplies: 2,
    utilities: 3,
    rent: 4,
    software: 5,
    professional_services: 6,
    equipment: 7,
    marketing: 8,
    other: 9
  }, prefix: :expense

  monetize :amount_cents, with_currency: :eur, allow_nil: true
  monetize :tax_amount_cents, with_currency: :eur, allow_nil: true
  monetize :opening_balance_cents, with_currency: :eur, allow_nil: true
  monetize :closing_balance_cents, with_currency: :eur, allow_nil: true

  validates :document_type, presence: true
  validates :ai_status, presence: true
  validates :review_status, presence: true
  validates :source, presence: true
  validates :original_file, presence: true, on: :create

  before_save :sync_document_type_id
  before_save :auto_star_for_type, if: :document_type_id_changed?

  scope :for_month, ->(year, month) {
    start_date = Date.new(year, month, 1)
    end_date = start_date.end_of_month
    where(document_date: start_date..end_date)
  }

  scope :by_type, ->(type_id) { where(document_type_id: type_id) if type_id.present? }
  scope :by_category, ->(category) {
    if category.present?
      type_ids = DocumentType.where(category: category).pluck(:id)
      where(document_type_id: type_ids)
    end
  }
  scope :by_review_status, ->(s) { where(review_status: s) if s.present? }
  scope :by_ai_status, ->(s) { where(ai_status: s) if s.present? }
  # Docs awaiting human sign-off: the AI finished (so there's a classification to
  # approve) but no one has approved/rejected yet. Drives the review notification
  # count (Notifier), the Skim feed (SkimScope) and the list-view review filter, so a
  # document leaves every "needs review" surface the moment it's approved or rejected.
  scope :needs_review, -> { where(review_status: :pending, ai_status: :completed) }
  # Attachments that aren't reviewable business documents — calendar invites, raw
  # emails, archives, DMARC/feedback reports. They carry no extractable data and only
  # clutter the Skim review queue, so SkimScope filters them out. A subquery (rather
  # than a join) keeps this composable with `includes`/`with_attached_original_file`;
  # documents with no attachment are unaffected.
  NON_DOCUMENT_CONTENT_TYPES = %w[
    text/calendar message/rfc822
    application/zip application/gzip application/x-gzip application/x-bzip2
    application/xhtml+xml text/html
  ].freeze
  scope :reviewable_attachment, -> {
    where.not(
      id: joins(original_file_attachment: :blob)
            .where(ActiveStorage::Blob.table_name => { content_type: NON_DOCUMENT_CONTENT_TYPES })
            .select(:id)
    )
  }
  # AI broke (adapter/parse/config error) — a separate "needs attention" lane. These
  # have no classification to approve; the human action is reprocess or manual-classify.
  scope :ai_failed_attention, -> { where(ai_status: :failed) }
  # Docs the "reanalyze" actions can re-run: anything still awaiting human sign-off and
  # not currently being processed (covers never-run, AI-failed, and completed-pending).
  scope :reprocessable, -> { where(review_status: :pending).where.not(ai_status: :processing) }
  scope :recent, -> { order(created_at: :desc) }
  # Surfaces starred documents at the top of the list, newest-first within each group.
  scope :starred_first, -> { order(starred: :desc) }
  scope :pushed_to_drive, -> { where(google_drive_push_status: :pushed) }
    scope :by_organization, ->(org, active_only: true) {
    people_ids = Person.joins(:organization_memberships)
      .where(organization_memberships: { organization_id: org.id })
    people_ids = people_ids.where(organization_memberships: { status: :active }) if active_only
    contact_ids = Contact.where(person_id: people_ids.select(:id)).select(:id)
    joins(:document_email_messages)
      .where(document_email_messages: { email_message_id: EmailMessage.where(contact_id: contact_ids).select(:id) })
      .distinct
  }

  scope :not_pushed_to_drive, -> { where(google_drive_push_status: [ :not_pushed, :failed ]) }

  def generate_canonical_filename!
    self.canonical_filename = Documents::FilenameGenerator.new(self).call
    save!
  end

  # --- Skim review actions (shared by DocumentsController and Documents::SkimController) ---

  # Confirm the AI's classification: the human signs off (review_status: approved).
  # The AI axis is untouched. Auto-pushing to Drive is the caller's concern (the
  # controller defers it during Skim so Undo can cancel).
  def approve!(by:)
    update!(review_status: :approved, reviewed_by: by, reviewed_at: Time.current)
    Events.publish("document.approved", subject: self, actor: by, payload: tracking_payload)
  end

  # Re-file under a different type. Picking the right type is itself the human
  # review, so this signs the document off in the same step — it becomes :approved
  # and leaves the "needs review" surfaces, exactly like #approve!. (Auto-pushing to
  # Drive is the caller's concern, deferred during Skim so Undo can cancel it.)
  # Setting document_type_id directly mirrors the edit form; the legacy
  # `document_type` enum is left as-is (the only sync hook, sync_document_type_id,
  # runs enum -> id, never the reverse).
  def reclassify!(document_type, by:)
    update!(
      document_type_id: document_type.id,
      review_status: :approved,
      reviewed_by: by,
      reviewed_at: Time.current
    )
    Events.publish("document.approved", subject: self, actor: by, payload: tracking_payload)
  end

  # Reject: flag as junk / not-a-doc / wrong — drops out of the review queue without
  # deleting (reversible via #restore!). Uses update_columns so it skips
  # validations/callbacks and won't re-trigger search reindexing (the document's
  # content is unchanged). The raw `2` is review_status: :rejected — update_columns
  # doesn't resolve enum names.
  def reject!
    update_columns(review_status: 2, updated_at: Time.current)
    Events.publish("document.rejected", subject: self, payload: tracking_payload)
  end

  # Undo: return the document to the human review queue regardless of how it left it
  # (approved or rejected). Clearing the reviewer stamps un-does an approve; resetting
  # review_status un-does a rejection — one method covers the Undo for both. The AI
  # axis is untouched: the analysis is still valid, only the human decision is reset.
  def restore!
    update!(review_status: :pending, reviewed_by: nil, reviewed_at: nil)
    Events.publish("document.restored", subject: self, payload: tracking_payload)
  end

  # Single, two-axis-aware status for badges/filters: the AI lifecycle wins while it's
  # still working or broken; otherwise the human review state (pending/approved/
  # rejected) is what the user cares about.
  def display_status
    return "processing" if ai_processing?
    return "failed" if ai_failed?
    review_status
  end

  def image?
    original_file.content_type&.start_with?("image/")
  end

  def pdf?
    original_file.content_type == "application/pdf"
  end

  def needs_pdf_conversion?
    image? && !processed_pdf.attached?
  end

  def display_title
    metadata&.dig("title").presence || entity_display_name.presence || original_file.filename.to_s
  end

  # The user-editable display name lives in metadata["title"] (the first source for
  # #display_title). Blank clears it, so the name falls back to the entity/filename.
  # Assign-only — the caller saves (so it can batch with other field edits).
  def assign_title(value)
    self.metadata = (metadata || {}).merge("title" => value.to_s.strip.presence)
  end

  def pushed_to_drive?
    google_drive_push_status == "pushed"
  end

  def drive_push_failed?
    google_drive_push_status == "failed"
  end

  def google_drive_url
    return nil unless google_drive_file_id.present?
    # google_drive_file_id is a Drive API file id (never user input), so this is
    # always a safe https://drive.google.com link. Returned as a plain string —
    # Rails escapes it normally when it's used as a link href.
    "https://drive.google.com/file/d/#{google_drive_file_id}/view"
  end

  def entity_display_name
    # Prefer sender_name (from email), then metadata, fall back to columns
    m = metadata.presence || {}
    sender_name.presence ||
      m["client_name"] || m["insured_party"] || m["counterparty"] ||
      m["person_name"] || m["subject_name"] || m["insurer_name"] ||
      m["sender"] || m["entity_name"] || m["proposer_name"] ||
      m["vendor_name"] || client_name || bank_name || vendor_name
  end

  def reference_display
    m = metadata.presence || {}
    case classification&.name
    when "bank_statement"
      ps = m["period_start"] || period_start
      pe = m["period_end"] || period_end
      if ps && pe
        begin
          "#{Date.parse(ps.to_s).strftime('%d/%m/%Y')} - #{Date.parse(pe.to_s).strftime('%d/%m/%Y')}"
        rescue
          nil
        end
      end
    when "bank_journal_entry"
      m["movement_type"] || m["counterparty_name"] || m["description"]&.truncate(30)
    when "receipt"
      m["receipt_number"] || receipt_number
    when "insurance_policy"
      m["policy_number"]
    when "certificate"
      m["certificate_number"]
    when "vehicle_document"
      m["plate_number"] || m["vin"]
    when "contract"
      m["contract_type"]
    else
      m["invoice_number"] || m["receipt_number"] || m["policy_number"] ||
        m["certificate_number"] || m["id_number"] || invoice_number ||
        receipt_number
    end
  end

  # --- Searchable concern implementation ---

  def build_search_chunks
    chunks = []

    # Primary description chunk
    primary = [
      ("Title: #{display_title}"),
      ("Type: #{classification&.name || document_type}"),
      ("Description: #{description}"),
      ("Vendor: #{vendor_name}"),
      ("Client: #{client_name}"),
      ("Invoice: #{invoice_number}")
    ].compact.join("
")

    chunks << {
      content: primary,
      chunk_type: "text",
      metadata: { field: "primary" }
    }

    # AI extraction data as separate chunk if present
    if ai_extraction_data.present?
      extraction_text = ai_extraction_data.is_a?(Hash) ? ai_extraction_data.to_json : ai_extraction_data.to_s
      chunks << {
        content: extraction_text,
        chunk_type: "text",
        metadata: { field: "ai_extraction" }
      }
    end

    # Splitting logic for very long descriptions
    expanded = []
    chunks.each do |chunk|
      if chunk[:content].length > 5000
        paragraphs = chunk[:content].split(/

+/)
        current = ""
        paragraphs.each do |para|
          if (current.length + para.length) > 5000 && current.present?
            expanded << { content: current.strip, chunk_type: chunk[:chunk_type], metadata: chunk[:metadata] }
            current = para
          else
            current += (current.empty? ? "" : "

") + para
          end
        end
        expanded << { content: current.strip, chunk_type: chunk[:chunk_type], metadata: chunk[:metadata] } if current.present?
      else
        expanded << chunk
      end
    end

    expanded
  end

  def searchable_title
    display_title
  end

  def searchable_content_preview
    description.presence || "#{classification&.name || document_type}: #{display_title}"
  end

  def searchable_filter_data
    {
      document_type: document_type,
      classification: classification&.name,
      ai_status: ai_status,
      review_status: review_status,
      source: source,
      document_date: document_date&.iso8601,
      vendor_name: vendor_name,
      client_name: client_name,
      invoice_number: invoice_number,
      amount_cents: amount_cents,
      email_account_id: email_account_id
    }.compact
  end

  def searchable_tags
    classification.present? ? [ classification.name ] : []
  end

  def searchable_source_created_at
    document_date || created_at
  end

  def searchable_fields_changed?
    saved_change_to_description? || saved_change_to_ai_extraction_data? ||
      saved_change_to_vendor_name? || saved_change_to_client_name? ||
      saved_change_to_document_type? || saved_change_to_document_type_id? ||
      saved_change_to_review_status? || saved_change_to_ai_status?
  end

  private

  # Thin metadata for document.* events.
  def tracking_payload
    { "filename" => original_file&.filename.to_s, "document_type" => (classification&.name || document_type) }
  end

  def sync_document_type_id
    return unless document_type_changed?
    dt = DocumentType.find_by(name: document_type)
    self.document_type_id = dt&.id
  end

  # Auto-star a document when its classification opts into it (DocumentType#auto_star).
  # Runs whenever the type changes — AI classification, manual reclassify, or edit — but
  # only ever *adds* a star: removing one stays a deliberate user action.
  def auto_star_for_type
    return if starred?
    self.starred = true if DocumentType.where(id: document_type_id, auto_star: true).exists?
  end
end
