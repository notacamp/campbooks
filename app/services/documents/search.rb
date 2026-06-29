module Documents
  # Files/documents search & filter query object — the single entry point behind the
  # Files search bar. Builds a flat, permission-scoped Document result set in either
  # browse (filter-only) or text-query (relevance) mode, mirroring Emails::Search.
  #
  # Permission is enforced by Document.accessible_to(user) in BOTH modes, and in
  # text-query mode the ranked ids coming back from the vector index are re-filtered
  # through that scope (and every structural filter + folder) in SQL — so a stale or
  # over-broad search index can only ever *narrow* the candidate set, never widen it
  # or leak a document the user (or this folder) shouldn't see.
  #
  # The query is run through Documents::QueryParser to pull soft hints out of natural
  # language: an unambiguous document_type pre-narrows the semantic candidate pool,
  # and a counterparty / number sharpens the keyword (ILIKE) arm. A wrong parse never
  # hides a result — the keyword arm always runs unfiltered by the parser.
  class Search
    # A free-text query returns a single bounded, rank-ordered page — HNSW has no
    # stable offset, so relevance results don't paginate. This caps the pool.
    RESULT_LIMIT = 100

    # @param folder [MailFolder, nil] when set, search is scoped to that folder.
    def initialize(user:, workspace:, params:, folder: nil)
      @user = user
      @workspace = workspace
      @params = params || {}
      @folder = folder
    end

    # A free-text query searches by relevance (embedding + keyword); with no query we
    # just browse/filter. Drives bounded-vs-paginated in the controller.
    def text_query?
      query.present?
    end

    # Browse / filter-only path. An AR relation safe for pagy.
    def scope
      apply_structural_filters(base_scope).starred_first.recent
    end

    # The free-text result set: a bounded, rank-ordered Array<Document> blending
    # embedding similarity with exact keyword matches. Embedding hits lead (by
    # similarity); literal keyword matches the vector missed are appended (starred /
    # recent first), so an exact lookup is never lost. When the index is empty or the
    # embedding call fails, semantic_ids is nil and this degrades to keyword only.
    # Every filter + permission + folder is re-applied in SQL on the merged id set.
    def results
      ids = ((semantic_ids || []) + keyword_ids).uniq.first(RESULT_LIMIT)
      return [] if ids.empty?

      by_id = apply_structural_filters(base_scope).where(id: ids).index_by(&:id)
      ids.filter_map { |id| by_id[id] }
    end

    # Ids of literal keyword matches, already filtered + permission-scoped + ordered.
    def keyword_ids
      keyword_scope.limit(RESULT_LIMIT).pluck(:id)
    end

    private

    def query
      @params[:q].to_s.strip.presence
    end

    def parsed
      @parsed ||= Documents::QueryParser.parse(query.to_s)
    end

    # Workspace + permission + (optional) folder. in_folder scopes to a SINGLE folder,
    # where FolderMembership is unique per (folder, document), so the join can't
    # duplicate a row — no .distinct needed (and DISTINCT would clash with the
    # starred/recent ORDER BY when we pluck only ids).
    def base_scope
      rel = @workspace.documents.accessible_to(@user)
      rel = rel.in_folder(@folder.id) if @folder
      rel
    end

    # User-explicit UI filters only — NOT the parser's hints, which stay soft.
    def apply_structural_filters(rel)
      rel = rel.by_type(@params[:type])
      rel = rel.by_category(@params[:category])
      rel = rel.by_review_status(@params[:review_status])
      rel = rel.by_ai_status(@params[:ai_status])
      rel = rel.for_month(*month_filter) if month_filter
      rel
    end

    # Keyword arm: structural filters + ILIKE across the user-visible text columns,
    # plus a targeted ILIKE on any counterparty / number the parser extracted (so an
    # entity buried in a noisy phrase still lands an exact hit on the right column).
    def keyword_scope
      apply_text_filter(apply_structural_filters(base_scope)).starred_first.recent
    end

    def apply_text_filter(rel)
      clauses = [
        "documents.vendor_name ILIKE :q", "documents.client_name ILIKE :q",
        "documents.description ILIKE :q", "documents.ai_summary ILIKE :q",
        "documents.invoice_number ILIKE :q", "documents.receipt_number ILIKE :q",
        "documents.canonical_filename ILIKE :q", "documents.metadata ->> 'title' ILIKE :q"
      ]
      binds = { q: "%#{sanitize_like(query)}%" }

      if parsed.counterparty.present?
        clauses << "documents.vendor_name ILIKE :cp" << "documents.client_name ILIKE :cp"
        binds[:cp] = "%#{sanitize_like(parsed.counterparty)}%"
      end
      if parsed.number.present?
        clauses << "documents.invoice_number ILIKE :num" << "documents.receipt_number ILIKE :num"
        binds[:num] = "%#{sanitize_like(parsed.number)}%"
      end

      rel.where(clauses.join(" OR "), binds)
    end

    # --- Meaning mode ---

    # Ranked Document ids from the vector index, scoped to documents.
    #   nil => index empty or search errored (caller falls back to keyword scope)
    #   []  => searched fine, nothing matched
    def semantic_ids
      return nil unless SearchRecord.where(workspace_id: @workspace.id).exists?

      raw = SearchService.search(
        parsed.cleaned_query,
        workspace: @workspace,
        filters: search_service_filters,
        # Documents aren't recency-ranked: a years-old contract is exactly what
        # people search for, so we keep date-range hints but disable score decay.
        options: { limit: RESULT_LIMIT, enable_temporal_scoring: false }
      )
      return [] if raw.blank?

      raw.filter_map { |r| r.searchable_id if r.searchable_type == "Document" }
    rescue => e
      Rails.logger.warn("[Documents::Search] meaning-mode search failed, falling back to keyword: #{e.message}")
      nil
    end

    # Only the searchable_type and an UNAMBIGUOUS parsed document_type pre-narrow the
    # vector pool. Counterparty/number deliberately stay out — a wrong parse must not
    # exclude a candidate before ranking can save it (the keyword arm carries them).
    def search_service_filters
      filters = { searchable_type: "Document" }
      filters[:document_type] = parsed.document_type if parsed.document_type.present?
      filters
    end

    # --- helpers ---

    def month_filter
      return if @params[:month].blank?
      date = Date.parse("#{@params[:month]}-01")
      [ date.year, date.month ]
    rescue ArgumentError
      nil
    end

    def sanitize_like(str)
      Document.sanitize_sql_like(str.to_s)
    end
  end
end
