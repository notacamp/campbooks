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
  #
  # Modifier tokens in the query string (`type:receipt vendor:EDP amount>100`) are
  # extracted by Documents::SearchQuery and promoted to hard SQL filters via
  # Documents::Filters. A query consisting ONLY of modifiers (no free text) falls
  # through to browse/pagination mode — text_query? is false in that case.
  class Search
    # A free-text query returns a single bounded, rank-ordered page — HNSW has no
    # stable offset, so relevance results don't paginate. This caps the pool.
    RESULT_LIMIT = 100

    # Exposed for controllers and views.
    attr_reader :filters, :sorter

    # @param folder [MailFolder, nil] when set, search is scoped to that folder.
    def initialize(user:, workspace:, params:, folder: nil)
      @user      = user
      @workspace = workspace
      @params    = params || {}
      @folder    = folder
      @filters   = build_filters
      @sorter    = build_sorter
    end

    # True when free text remains after stripping modifiers. A query that contains
    # only modifiers (e.g. "type:receipt is:pending") returns false and the
    # controller falls back to paginated browse mode.
    def text_query?
      search_text.present?
    end

    # The parsed free text (modifiers stripped) — exposed for the view's count line.
    def search_text
      @search_text ||= parsed_query.text
    end

    # Browse / filter-only path. An AR relation safe for pagy. Includes the
    # attachments and classification so the list renders without N+1s.
    # When a sorter is active its ORDER BY replaces the default starred_first/recent.
    def scope
      filtered = @filters.apply(base_scope, workspace: @workspace, user: @user)
                         .includes(:classification).with_attached_original_file

      if @sorter.active?
        @sorter.apply(filtered)
      else
        filtered.starred_first.recent
      end
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

      by_id = @filters.apply(base_scope, workspace: @workspace, user: @user)
                      .where(id: ids).index_by(&:id)
      ids.filter_map { |id| by_id[id] }
    end

    # Ids of literal keyword matches, already filtered + permission-scoped + ordered.
    def keyword_ids
      keyword_scope.limit(RESULT_LIMIT).pluck(:id)
    end

    private

    # ── Filter setup ─────────────────────────────────────────────────────────

    def build_filters
      base = Documents::Filters.from_params(@params)
      base.merge_query(parsed_query.filters) if raw_query.present?
      base
    end

    # Builds a Sorter. Extracted-field sort keys are only resolved when filters
    # narrow to exactly one DocumentType (schema is then unambiguous).
    def build_sorter
      dt = @filters.single_type(@workspace)
      Documents::Sorter.from_params(@params, document_type: dt)
    end

    # ── Query parsing ─────────────────────────────────────────────────────────

    def raw_query
      @raw_query ||= @params[:q].to_s.strip.presence
    end

    # Modifier-aware tokeniser — splits raw q into free text + filter directives.
    def parsed_query
      @parsed_query ||= Documents::SearchQuery.parse(raw_query.to_s)
    end

    # NL-hint parser on the MODIFIER-STRIPPED text (so type/number/counterparty
    # hints don't double-count with explicit modifier filters).
    def parsed
      @parsed ||= Documents::QueryParser.parse(search_text.to_s)
    end

    # ── Scopes ────────────────────────────────────────────────────────────────

    # Workspace + permission + (optional) folder. in_folder scopes to a SINGLE folder,
    # where FolderMembership is unique per (folder, document), so the join can't
    # duplicate a row — no .distinct needed (and DISTINCT would clash with the
    # starred/recent ORDER BY when we pluck only ids).
    def base_scope
      rel = @workspace.documents.accessible_to(@user)
      rel = rel.in_folder(@folder.id) if @folder
      rel
    end

    # Keyword arm: filters + ILIKE across the user-visible text columns,
    # plus a targeted ILIKE on any counterparty / number the parser extracted.
    def keyword_scope
      apply_text_filter(@filters.apply(base_scope, workspace: @workspace, user: @user))
        .starred_first.recent
    end

    def apply_text_filter(rel)
      return rel unless search_text.present?

      clauses = [
        "documents.metadata->>'vendor_name' ILIKE :q", "documents.metadata->>'client_name' ILIKE :q",
        "documents.description ILIKE :q", "documents.ai_summary ILIKE :q",
        "documents.metadata->>'invoice_number' ILIKE :q", "documents.metadata->>'receipt_number' ILIKE :q",
        "documents.canonical_filename ILIKE :q", "documents.metadata->>'title' ILIKE :q"
      ]
      binds = { q: "%#{sanitize_like(search_text)}%" }

      if parsed.counterparty.present?
        clauses << "documents.metadata->>'vendor_name' ILIKE :cp" << "documents.metadata->>'client_name' ILIKE :cp"
        binds[:cp] = "%#{sanitize_like(parsed.counterparty)}%"
      end
      if parsed.number.present?
        clauses << "documents.metadata->>'invoice_number' ILIKE :num" << "documents.metadata->>'receipt_number' ILIKE :num"
        binds[:num] = "%#{sanitize_like(parsed.number)}%"
      end

      rel.where(clauses.join(" OR "), binds)
    end

    # ── Semantic arm ─────────────────────────────────────────────────────────

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
      sf = { searchable_type: "Document" }
      sf[:document_type] = parsed.document_type if parsed.document_type.present?
      sf
    end

    # ── helpers ──────────────────────────────────────────────────────────────

    def sanitize_like(str)
      Document.sanitize_sql_like(str.to_s)
    end
  end
end
