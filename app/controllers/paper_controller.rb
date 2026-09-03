# frozen_string_literal: true

# Paper — the rethought document surface (Rethink stage 3). A file browser shows what a
# file is CALLED; Paper shows what it SAYS: every document is a row of facts (kind, amount,
# the date that matters, a meaning-bearing status). Reuses the Files search stack
# (Documents::Search + Files::Searchable) and renders the same permission-scoped documents
# through the Paper table. Gated on Features.bold_layout? (the flag, not the per-user
# preference — a classic-mode user can open /paper on a flag-on build); "Classic files"
# links back to /files.
class PaperController < ApplicationController
  include Pagy::Backend
  include Files::Searchable

  before_action :require_bold_layout_enabled

  PAGE_SIZE = 30

  # The filter chips are BUCKETS of document types (per the mock), not single types.
  BUCKET_TYPES = {
    invoices:  %w[expense_invoice revenue_invoice credit_note],
    receipts:  %w[receipt],
    contracts: %w[contract insurance_policy]
  }.freeze
  BUCKETS = ([ :all ] + BUCKET_TYPES.keys + [ :other ]).freeze

  # The ask box understands a few derived-status words. Because a document's status is
  # DERIVED (not a column), these can't be a SQL filter — they post-filter the result set
  # in Ruby (see #filter_statuses). Word boundaries keep "unpaid" from also matching "paid".
  STATUS_WORDS = {
    "needs review" => %i[needs_review],
    "unpaid"       => %i[unpaid late],
    "overdue"      => %i[late],
    "late"         => %i[late],
    "expiring"     => %i[expiring],
    "expired"      => %i[expired],
    "paid"         => %i[paid]
  }.freeze

  # Order documents by "the date that matters" (due, else contract end, else document date,
  # else when it landed), newest first. The date lives in the metadata JSONB, so it's a
  # guarded cast, not a column.
  MATTER_DATE_SQL = <<~SQL.squish
    COALESCE(
      CASE WHEN documents.metadata->>'due_date'      ~ '^\\d{4}-\\d{2}-\\d{2}' THEN (documents.metadata->>'due_date')::date END,
      CASE WHEN documents.metadata->>'period_end'    ~ '^\\d{4}-\\d{2}-\\d{2}' THEN (documents.metadata->>'period_end')::date END,
      CASE WHEN documents.metadata->>'document_date' ~ '^\\d{4}-\\d{2}-\\d{2}' THEN (documents.metadata->>'document_date')::date END,
      documents.created_at::date
    )
  SQL

  def index
    @bucket = resolve_bucket
    @requested_statuses = requested_statuses

    # Strip the status words out of the query the search runs on (they're post-filtered),
    # and never let Paper's bucket `type` param reach Documents::Filters (it expects
    # document_type UUIDs, not bucket keys).
    build_search(search_params: params.except(:type).merge(q: residual_query))

    @document_types = Current.workspace ? Current.workspace.document_types.order(:name) : []
    @folders = Current.workspace ? Current.workspace.mail_folders.accessible_to(Current.user).ordered.to_a : []
    @needs_review_count = Current.workspace ? Current.workspace.documents.needs_review.count : 0
    @summary = Documents::Summary.for(Current.workspace, Current.user)
    @bucket_counts = bucket_counts
    @has_any = Current.workspace&.documents&.exists? || false

    @documents, @pagy = load_documents

    # Visiting Paper clears the "new documents" nav dot (mirrors Files).
    Current.workspace&.documents&.needs_review&.where(viewed_at: nil)&.update_all(viewed_at: Time.current)

    respond_to do |format|
      format.html
      format.turbo_stream # the lazy pagination append (index.turbo_stream.erb)
    end
  end

  private

  def resolve_bucket
    bucket = params[:type].to_s.to_sym
    BUCKETS.include?(bucket) ? bucket : :all
  end

  # The visible page of documents + a pagy (nil in text-query mode, which is a single
  # bounded, rank-ordered page — HNSW has no stable offset, same as Files).
  def load_documents
    if @search.text_query?
      docs = filter_statuses(filter_bucket(@search.results))
      [ docs, nil ]
    else
      scope = order_by_matter_date(bucket_scope(@search.scope))
      pagy, page = pagy(scope, items: PAGE_SIZE)
      # Statuses are derived, not indexed: in browse mode a status word can only narrow
      # WITHIN the fetched page. In practice the ask box's status words arrive with free
      # text (the bounded text-query path above), so this is a rare, acceptable edge.
      [ filter_statuses(page.to_a), pagy ]
    end
  end

  def bucket_scope(scope)
    return scope if @bucket == :all

    scope.where(document_type: bucket_types(@bucket))
  end

  def order_by_matter_date(scope)
    scope.reorder(Arel.sql("#{MATTER_DATE_SQL} DESC NULLS LAST, documents.created_at DESC"))
  end

  def filter_bucket(docs)
    return docs if @bucket == :all

    types = bucket_types(@bucket)
    docs.select { |doc| types.include?(doc.document_type) }
  end

  def filter_statuses(docs)
    return docs if @requested_statuses.empty?

    docs.select { |doc| @requested_statuses.include?(Documents::Status.for(doc).status) }
  end

  def bucket_types(bucket)
    return other_types if bucket == :other

    BUCKET_TYPES.fetch(bucket, [])
  end

  def other_types
    Document.document_types.keys - BUCKET_TYPES.values.flatten
  end

  # Chip counts: one grouped pass over the filtered (but pre-bucket) scope, mapped to the
  # buckets. Reflects the active structural filters, not the free-text/status narrowing.
  def bucket_counts
    return BUCKETS.index_with { 0 } unless Current.workspace

    scope = @filters.apply(
      Current.workspace.documents.accessible_to(Current.user),
      workspace: Current.workspace, user: Current.user
    )
    by_type = normalize_type_counts(scope.group(:document_type).count)

    {
      all:       by_type.values.sum,
      invoices:  bucket_sum(by_type, :invoices),
      receipts:  bucket_sum(by_type, :receipts),
      contracts: bucket_sum(by_type, :contracts),
      other:     bucket_sum(by_type, :other)
    }
  end

  # group(:document_type) can key by the enum integer or its label depending on the AR
  # path; normalise both to the label so the bucket mapping is stable.
  def normalize_type_counts(by_type)
    inverse = Document.document_types.invert
    by_type.each_with_object(Hash.new(0)) do |(key, count), acc|
      label = key.is_a?(Integer) ? inverse[key] : key.to_s
      acc[label] += count if label
    end
  end

  def bucket_sum(counts, bucket)
    bucket_types(bucket).sum { |type| counts[type].to_i }
  end

  def requested_statuses
    haystacks = [ params[:q].to_s.downcase, params[:status].to_s.downcase ]
    STATUS_WORDS.select do |word, _|
      haystacks.any? { |h| h.match?(/\b#{Regexp.escape(word)}\b/) }
    end.values.flatten.uniq
  end

  # The query with the status words removed — what the search actually runs on. Blank means
  # a status-only ask ("unpaid"), which falls to paginated browse + a Ruby status filter.
  def residual_query
    query = params[:q].to_s
    STATUS_WORDS.each_key { |word| query = query.gsub(/\b#{Regexp.escape(word)}\b/i, " ") }
    query.squish.presence
  end
end
