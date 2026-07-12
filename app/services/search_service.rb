class SearchService
  DEFAULT_OPTIONS = {
    limit: 20,
    similarity_threshold: 0.3,
    title_weight: 1.5,
    content_weight: 1.5,
    tag_boost_weight: 0.3,
    temporal_weight: 0.2,
    decay_days: 180,
    candidate_pool_multiplier: 5,
    max_candidates: 500,
    enable_tag_boosting: true,
    enable_temporal_scoring: true,
    detect_intent: true
  }.freeze

  Result = Struct.new(
    :searchable_type, :searchable_id, :search_record,
    :score, :title_similarity, :content_similarity, :recency_score,
    :matched_tags, keyword_init: true
  )

  def self.search(query, workspace:, filters: {}, options: {})
    new(workspace).search(query, filters: filters, options: options)
  end

  def initialize(workspace)
    @workspace = workspace
  end

  def search(query, filters: {}, options: {})
    return [] if query.blank?

    opts  = DEFAULT_OPTIONS.merge(options)
    entry = @workspace.embedding_model_entry

    # 1. Generate query embedding via the workspace's configured model
    query_embedding = EmbeddingService.embed(query, workspace: @workspace, entry: entry)
    return [] unless query_embedding

    # 2. Analyze query
    query_analysis = QueryAnalyzer.new(query, workspace: @workspace).analyze

    # 3. Build base scope with filters
    scope = SearchRecord.where(workspace_id: @workspace.id)
    scope = apply_filters(scope, filters, query_analysis)

    # 4. Vector search
    candidates = vector_search(scope, query_embedding, opts, filters, entry)

    # 5. Compute scores
    results = compute_scores(candidates, query_embedding, opts, entry: entry)

    # 6. Tag boosting
    if opts[:enable_tag_boosting] && query_analysis.matched_tags.any?
      results = apply_tag_boosting(results, query_analysis)
    end

    # 7. Temporal scoring
    if opts[:enable_temporal_scoring]
      results = apply_temporal_scoring(results, opts)
    end

    # 8. Sort and threshold
    results = results
      .select { |r| r.score >= opts[:similarity_threshold] }
      .sort_by { |r| -r.score }
      .first(opts[:limit])

    results
  end

  private

  def apply_filters(scope, filters, query_analysis)
    scope = scope.by_type(filters[:searchable_type]) if filters[:searchable_type].present?

    if filters[:tags].present?
      scope = scope.with_tags(filters[:tags])
    end

    if filters[:document_type].present?
      scope = scope.where("filter_data ->> 'document_type' = ?", filters[:document_type])
    end

    if filters[:status].present?
      scope = scope.where("filter_data ->> 'status' = ?", filters[:status])
    end

    if filters[:from_address].present?
      scope = scope.where("filter_data ->> 'from_address' ILIKE ?", "%#{filters[:from_address]}%")
    end

    if filters[:email].present?
      scope = scope.where("filter_data ->> 'email' ILIKE ?", "%#{filters[:email]}%")
    end

    # Email inbox filters (Emails::Search). All read off the GIN-indexed jsonb
    # filter_data. Folder/account accept arrays — a folder name maps to several
    # provider ids across accounts, and account is a multi-select.
    if filters[:provider_folder_ids].present?
      ids = Array(filters[:provider_folder_ids]).map(&:to_s)
      scope = scope.where("filter_data ->> 'provider_folder_id' = ANY(ARRAY[?]::text[])", ids)
    end

    if filters[:account_ids].present?
      ids = Array(filters[:account_ids]).map(&:to_s)
      scope = scope.where("filter_data ->> 'email_account_id' = ANY(ARRAY[?]::text[])", ids)
    end

    if filters[:unread].to_s == "true"
      scope = scope.where("filter_data ->> 'read' = ?", "false")
    end

    if filters[:category].present?
      scope = scope.where("filter_data ->> 'category' = ?", filters[:category])
    end

    if filters[:sender_domain].present?
      scope = scope.where("filter_data ->> 'sender_domain' = ?", filters[:sender_domain].to_s.downcase)
    end

    if filters[:has_attachments].to_s == "true"
      scope = scope.where("filter_data ->> 'has_attachments' = ?", "true")
    end

    if filters[:priority].present?
      scope = scope.where("filter_data ->> 'ai_priority' = ?", filters[:priority])
    end

    # Temporal filters from query analysis
    if query_analysis.temporal_hint
      hint = query_analysis.temporal_hint
      scope = scope.where("source_created_at >= ?", hint[:since]) if hint[:since]
      scope = scope.where("source_created_at <= ?", hint[:until]) if hint[:until]
    end

    # Temporal filters from explicit filters
    if filters[:date_from].present?
      scope = scope.where("source_created_at >= ?", filters[:date_from])
    end
    if filters[:date_to].present?
      scope = scope.where("source_created_at <= ?", filters[:date_to])
    end

    # Type intent from query analysis
    if query_analysis.intents.include?(:urgent)
      scope = scope.where("filter_data ->> 'ai_priority' = ?", "high")
    end

    scope
  end

  def vector_search(scope, query_embedding, opts, filters, entry)
    candidate_limit = [ opts[:limit] * opts[:candidate_pool_multiplier], opts[:max_candidates] ].min

    # Find nearest neighbors by content embedding using HNSW index.
    # fresh_for ensures only records stamped for the workspace's current model
    # are considered — stale records (wrong embedding space) are excluded.
    content_col = SearchRecord.embedding_column_for(:content_embedding, entry.dimensions)
    scope
      .merge(SearchRecord.fresh_for(entry, kind: :content_embedding))
      .nearest_neighbors(content_col, query_embedding, distance: "cosine")
      .limit(candidate_limit)
      .to_a
  end

  def compute_scores(candidates, query_embedding, opts, entry:)
    title_weight   = opts[:title_weight]
    content_weight = opts[:content_weight]

    candidates.map do |sr|
      title_sim   = cosine_similarity(sr.embedding_vector(:title_embedding, entry.dimensions), query_embedding) || 0.0
      content_sim = cosine_similarity(sr.embedding_vector(:content_embedding, entry.dimensions), query_embedding) || 0.0

      score = [ title_sim * title_weight, content_sim * content_weight ].max

      Result.new(
        searchable_type: sr.searchable_type,
        searchable_id: sr.searchable_id,
        search_record: sr,
        score: score,
        title_similarity: title_sim,
        content_similarity: content_sim,
        recency_score: 1.0,
        matched_tags: []
      )
    end
  end

  def apply_tag_boosting(results, query_analysis)
    return results if query_analysis.matched_tags.empty?

    tag_weight = query_analysis.tag_boost_weight
    tag_ids = query_analysis.matched_tags.map { |m| m[:tag].id }

    # Preload taggings for all candidate records
    searchables_by_type = {}
    results.each do |r|
      searchables_by_type[r.searchable_type] ||= []
      searchables_by_type[r.searchable_type] << r.searchable_id
    end

    # Build a set of (searchable_type, searchable_id) that have matching tags
    tagged_records = Set.new
    searchables_by_type.each do |type, ids|
      if type == "EmailMessage"
        EmailMessageTag.joins(:tag)
          .where(email_message_id: ids, tag_id: tag_ids)
          .pluck(:email_message_id)
          .each { |id| tagged_records << [ type, id ] }
      end
    end

    results.each do |r|
      query_analysis.matched_tags.each do |match|
        if tagged_records.include?([ r.searchable_type, r.searchable_id ])
          boost = tag_weight * match[:confidence]
          r.score *= (1.0 + boost)
          r.matched_tags << match[:tag].name
        end
      end
    end

    results
  end

  def apply_temporal_scoring(results, opts)
    temporal_weight = opts[:temporal_weight]
    decay_days = opts[:decay_days].to_f

    results.each do |r|
      source_date = r.search_record.source_created_at || Time.current
      days_old = (Time.current - source_date) / 1.day
      recency = Math.exp(-days_old / decay_days)

      r.recency_score = recency
      r.score = (1.0 - temporal_weight) * r.score + temporal_weight * recency
    end

    results
  end

  def cosine_similarity(vec_a, vec_b)
    return nil if vec_a.nil? || vec_b.nil? || vec_a.empty? || vec_b.empty?

    dot_product = 0.0
    magnitude_a = 0.0
    magnitude_b = 0.0

    vec_a.each_with_index do |a, i|
      b = vec_b[i] || 0.0
      dot_product += a * b
      magnitude_a += a * a
      magnitude_b += b * b
    end

    mag_product = Math.sqrt(magnitude_a) * Math.sqrt(magnitude_b)
    return 0.0 if mag_product.zero?

    dot_product / mag_product
  end
end
