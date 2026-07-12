# frozen_string_literal: true

# Spotlight-style aggregated search for the Cmd+K command palette.
#
# Semantic search (SearchService) drives the content-heavy types where meaning
# matters (emails, documents). Everything else uses fast ILIKE. We always fall
# back to ILIKE when the vector index is empty or the embedding call fails, so
# results still appear before embeddings are populated.
#
# Results are uniform hashes the palette renders directly:
#   { type:, id:, title:, subtitle:, icon:, url: }
# where `type` is the group header, `id` is the record id (used when a composite
# command needs to act on the picked record), and `icon` keys into the JS ICONS.
class GlobalSearch
  include Rails.application.routes.url_helpers

  MIN_QUERY_LENGTH = 2
  PER_GROUP_LIMIT = 5

  # Single-type lookups for composite-command argument slots ("pick an email",
  # "pick a tag"). Keyword (ILIKE) only — snappy and free per keystroke, which is
  # what an argument picker wants (the full palette search stays semantic).
  TYPE_METHODS = {
    "emails" => :ilike_email_results,
    "documents" => :ilike_document_results,
    "contacts" => :contact_results,
    "threads" => :thread_results,
    "tags" => :tag_results,
    "document_types" => :document_type_results,
    "workflows" => :workflow_results
  }.freeze

  def self.call(query, user:, types: nil)
    new(query, user: user, types: types).call
  end

  def initialize(query, user:, types: nil)
    @query = query.to_s.strip
    @user = user
    @workspace = user&.workspace
    @types = types.present? ? Array(types).map(&:to_s) : nil
  end

  def call
    return [] if @workspace.nil? || @query.length < MIN_QUERY_LENGTH
    return typed_results if @types

    [
      *email_and_document_results,
      *contact_results,
      *thread_results,
      *tag_results,
      *document_type_results,
      *workflow_results
    ]
  end

  private

  def typed_results
    @types.flat_map { |t| (method_name = TYPE_METHODS[t]) ? send(method_name) : [] }
  end

  def like
    @like ||= "%#{@query}%"
  end

  # --- Emails + documents: semantic first, ILIKE fallback ---

  def email_and_document_results
    semantic = semantic_results
    semantic.any? ? semantic : ilike_email_results + ilike_document_results
  end

  def semantic_results
    return [] unless SearchRecord.where(workspace_id: @workspace.id).exists?

    raw = SearchService.search(@query, workspace: @workspace, options: { limit: 20 })
    return [] if raw.blank?

    allowed_account_ids = @user.readable_email_accounts.ids.to_set
    counts = Hash.new(0)

    raw.filter_map do |r|
      sr = r.search_record
      next unless sr

      case r.searchable_type
      when "EmailMessage"
        next unless allowed_account_ids.include?(sr.filter_data["email_account_id"].to_i)
        next if (counts[:emails] += 1) > PER_GROUP_LIMIT

        email_result(r.searchable_id, sr.title, sr.filter_data["from_address"])
      when "Document"
        next if (counts[:documents] += 1) > PER_GROUP_LIMIT

        result("Documents", sr.title.presence || "Document", sr.content_preview, "file-text", document_path(r.searchable_id), id: r.searchable_id)
      end
    end
  rescue => e
    Rails.logger.warn("[GlobalSearch] semantic search failed, using ILIKE: #{e.message}")
    []
  end

  def ilike_email_results
    EmailMessage.where(email_account: @user.readable_email_accounts)
                .where("subject ILIKE :q OR from_address ILIKE :q", q: like)
                .order(received_at: :desc)
                .limit(PER_GROUP_LIMIT)
                .map { |m| email_result(m.id, m.subject, m.from_address) }
  end

  def ilike_document_results
    @workspace.documents
              .where("documents.metadata->>'vendor_name' ILIKE :q OR documents.metadata->>'client_name' ILIKE :q OR description ILIKE :q OR documents.metadata->>'invoice_number' ILIKE :q OR canonical_filename ILIKE :q", q: like)
              .order(created_at: :desc)
              .limit(PER_GROUP_LIMIT)
              .map { |d| result("Documents", d.display_title, d.entity_display_name, "file-text", document_path(d), id: d.id) }
  end

  # --- ILIKE-only types (not in the vector index) ---

  def contact_results
    @workspace.contacts
              .where("name ILIKE :q OR email ILIKE :q OR organization ILIKE :q", q: like)
              .order(:name)
              .limit(PER_GROUP_LIMIT)
              .map { |c| result("Contacts", c.display_name, c.email, "users", email_messages_path(inbox_settings: "contacts"), id: c.id) }
  end

  def thread_results
    @user.agent_threads.global
         .where("title ILIKE ?", like)
         .order(updated_at: :desc)
         .limit(PER_GROUP_LIMIT)
         .map { |t| result("Scout threads", t.title, "Scout AI chat", "sparkles", scout_thread_path(t), id: t.id) }
  end

  def tag_results
    @workspace.tags
              .where("name ILIKE ?", like)
              .order(:name)
              .limit(PER_GROUP_LIMIT)
              .map { |t| result("Tags", t.name, (t.group_name.presence && "Group: #{t.group_name}"), "tag", email_messages_path(inbox_settings: "tags"), id: t.id) }
  end

  def document_type_results
    @workspace.document_types
              .where("name ILIKE ?", like)
              .order(:name)
              .limit(PER_GROUP_LIMIT)
              .map { |dt| result("Document types", dt.name.to_s.humanize, dt.category, "file-text", email_messages_path(inbox_settings: "document_types"), id: dt.id) }
  end

  def workflow_results
    return [] unless Features.workflows?

    @workspace.workflows
              .where("name ILIKE ?", like)
              .order(:name)
              .limit(PER_GROUP_LIMIT)
              .map { |w| result("Workflows", w.name, w.trigger_type&.humanize, "workflow", workflow_path(w), id: w.id) }
  end

  # --- Result builders ---

  def email_result(id, subject, from_address)
    result("Emails", subject.presence || "(no subject)", from_address, "mail", email_message_path(id), id: id)
  end

  def result(type, title, subtitle, icon, url, id: nil)
    { type: type, id: id, title: title.to_s, subtitle: subtitle.presence, icon: icon, url: url }
  end
end
