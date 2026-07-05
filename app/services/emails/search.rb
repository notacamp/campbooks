# frozen_string_literal: true

module Emails
  # Inbox search & filter query object — the single entry point behind the email
  # search bar. Builds a flat, permission-scoped EmailMessage result set from the
  # search params in either keyword (ILIKE) or meaning (embedding) mode.
  #
  # Permission is enforced by EmailMessage.accessible_to(user) in BOTH modes. In
  # meaning mode the ranked ids from the vector index are additionally re-filtered
  # through that scope (and every structural filter) in SQL, so a stale or
  # over-broad search index can never leak a message the user may not see — the
  # vector index only ever *narrows* the candidate set, never widens it.
  #
  # Folder filtering takes pre-resolved provider_folder_ids: a folder name maps to
  # several provider ids across accounts, and that mapping is mail-client-backed
  # and cached in EmailMessagesController, so the controller resolves it and hands
  # the ids in (keeping this object free of network/cache concerns).
  #
  # RANKING (free-text mode):
  #   final = MATCH_WEIGHT * match + RECENCY_WEIGHT * recency
  # where match = max(semantic_score_normalized, keyword_field_weight) and
  # recency = exp(-age_days / RECENCY_DECAY_DAYS).  Match dominates (0.85 weight);
  # recency is a light tiebreak (0.15 weight) so a very recent low-relevance message
  # cannot outrank a highly relevant but older one.
  class Search
    # A free-text query runs a relevance search (embedding + keyword) and returns a
    # single bounded, rank-ordered page — HNSW has no stable offset, so relevance
    # results don't paginate. This caps the result/candidate pool.
    RESULT_LIMIT = 100

    # Union pool of semantic + keyword ids fetched before ranking. Generous so
    # the final RESULT_LIMIT still has good diversity after the ranking pass.
    CANDIDATE_LIMIT = 150

    # --- Ranking weights ---
    MATCH_WEIGHT         = 0.85
    RECENCY_WEIGHT       = 0.15
    # SearchService title/content weights (1.5) push cosine scores up to ~1.5;
    # we normalise back into [0, 1] before blending.
    SEMANTIC_SCORE_NORM  = 1.5
    RECENCY_DECAY_DAYS   = 90.0

    # Field-match scores for the keyword fallback path (mutually exclusive — the
    # highest matching field wins).
    KEYWORD_SUBJECT_SCORE = 0.95
    KEYWORD_FROM_SCORE    = 0.8
    KEYWORD_SUMMARY_SCORE = 0.65

    # @param folder_ids [Array<String>, nil] pre-resolved provider folder ids.
    #   nil => no folder filter; [] => a folder was chosen but matched nothing.
    def initialize(user:, params:, folder_ids: nil)
      @user      = user
      @params    = params || {}
      @folder_ids = folder_ids
      @parsed    = Emails::SearchQuery.parse(@params[:q])
    end

    # A free-text query searches by relevance (embedding similarity + keyword); with
    # no query we just browse/filter. Drives bounded-vs-paginated in the controller.
    def text_query?
      query.present?
    end

    # Keyword / filter-only path. An AR relation safe for pagy_countless.
    def scope
      apply_text_filter(apply_structural_filters(base_scope)).order(received_at: :desc)
    end

    # The free-text result set: a bounded, rank-ordered Array<EmailMessage> blending
    # embedding similarity with keyword field weights and a recency decay.
    # Embedding hits lead by match score; any literal keyword matches the vector
    # search missed are included and ranked by their field weight. When the index is
    # empty or the embedding call fails, semantic_scores is nil and this degrades to
    # keyword-only. All filters + permission are re-applied in SQL on the merged
    # id set.
    def results
      sem = semantic_scores # nil (fallback) | Hash{id => raw_score}
      ids = (((sem&.keys || []) + keyword_ids).uniq).first(CANDIDATE_LIMIT)
      return [] if ids.empty?

      by_id   = apply_structural_filters(base_scope).where(id: ids).index_by(&:id)
      q_lower = query&.downcase

      ranked = by_id.values.map do |m|
        [ m, blended_score(m, sem, q_lower) ]
      end

      ranked
        .sort_by { |m, score| [ -score, -(m.received_at&.to_i || 0) ] }
        .first(RESULT_LIMIT)
        .map(&:first)
    end

    # Ids of literal keyword matches (subject/from/ai_summary), already filtered +
    # permission-scoped + recency-ordered (scope), capped to the result pool.
    def keyword_ids
      scope.limit(RESULT_LIMIT).pluck(:id)
    end

    private

    # The free-text remainder after modifier tokens are stripped.
    def query
      @parsed.text.presence
    end

    def base_scope
      EmailMessage.accessible_to(@user)
    end

    # --- Score blending ---

    def blended_score(message, semantic_scores, q_lower)
      sem_raw = semantic_scores&.[](message.id) || 0
      semantic = [ sem_raw / SEMANTIC_SCORE_NORM, 1.0 ].min

      keyword = if q_lower.present?
        subject_hit = message.subject.to_s.downcase.include?(q_lower)
        from_hit    = message.from_address.to_s.downcase.include?(q_lower)
        summary_hit = message.ai_summary.to_s.downcase.include?(q_lower)

        if subject_hit then KEYWORD_SUBJECT_SCORE
        elsif from_hit then KEYWORD_FROM_SCORE
        elsif summary_hit then KEYWORD_SUMMARY_SCORE
        else 0.0
        end
      else
        0.0
      end

      age_days = (Time.current - (message.received_at || Time.current)) / 1.day
      recency  = Math.exp(-age_days / RECENCY_DECAY_DAYS)

      match = [ semantic, keyword ].max
      MATCH_WEIGHT * match + RECENCY_WEIGHT * recency
    end

    # --- Structural (SQL) filters, applied in both modes ---

    def apply_structural_filters(rel)
      rel = filter_folder(rel)
      rel = filter_accounts(rel)
      rel = filter_tags(rel)
      rel = filter_sender(rel)
      rel = filter_domain(rel)
      rel = filter_recipient(rel)
      rel = filter_subject(rel)
      rel = filter_dates(rel)
      rel = filter_attachment(rel)
      rel = filter_unread(rel)
      rel = filter_read(rel)
      rel = filter_pinned(rel)
      rel = filter_category(rel)
      rel = filter_priority(rel)
      filter_account_query(rel)
    end

    def apply_text_filter(rel)
      return rel if query.blank?
      like = "%#{sanitize_like(query)}%"
      # Body is intentionally excluded — it's large provider HTML and an unindexed
      # ILIKE on it would scan the table. Deep body content is reachable via
      # meaning mode (embeddings are built from the body); ai_summary covers the
      # gist for keyword mode.
      rel.where("subject ILIKE :q OR from_address ILIKE :q OR ai_summary ILIKE :q", q: like)
    end

    def filter_folder(rel)
      return rel if @folder_ids.nil?
      rel.where(provider_folder_id: @folder_ids) # [] => no rows, which is correct
    end

    def filter_accounts(rel)
      ids = id_array(@params[:account_ids])
      return rel if ids.empty?
      allowed = @user.readable_email_accounts.where(id: ids).ids
      allowed.any? ? rel.where(email_account_id: allowed) : rel.none
    end

    def filter_tags(rel)
      ids = id_array(@params[:tag_ids])
      parsed_names = @parsed.filters[:tag_names] || []

      # Resolve parsed tag names within the workspace (case-insensitive). A name
      # may map to several tag rows (provider tags share names across accounts) —
      # keep them grouped so a name matches ANY of its rows, while distinct names
      # still AND together (Gmail label semantics).
      parsed_id_groups = parsed_names.map do |name|
        Tag.where(workspace_id: @user.workspace_id)
           .where("LOWER(name) = ?", name.downcase)
           .ids
      end

      # An unresolvable tag name → rel.none (predictable "no results").
      return rel.none if parsed_id_groups.any?(&:empty?)

      # Panel tag_ids — use existing any/all logic.
      if ids.any?
        if @params[:tag_match].to_s == "all"
          rel = ids.reduce(rel) do |scoped, tag_id|
            scoped.where(
              "EXISTS (SELECT 1 FROM email_message_tags emt " \
              "WHERE emt.email_message_id = email_messages.id AND emt.tag_id = ?)",
              tag_id
            )
          end
        else
          rel = rel.joins(:email_message_tags).where(email_message_tags: { tag_id: ids }).distinct
        end
      end

      # Parsed tag names — one EXISTS per name (AND across names, OR within the
      # rows sharing that name).
      parsed_id_groups.each do |tag_ids|
        rel = rel.where(
          "EXISTS (SELECT 1 FROM email_message_tags emt " \
          "WHERE emt.email_message_id = email_messages.id AND emt.tag_id IN (?))",
          tag_ids
        )
      end

      rel
    end

    def filter_sender(rel)
      values = [
        (@params[:sender].to_s.strip.presence),
        *(@parsed.filters[:sender] || [])
      ].compact
      values.reduce(rel) do |r, v|
        r.where("from_address ILIKE ?", "%#{sanitize_like(v)}%")
      end
    end

    def filter_domain(rel)
      values = []
      if (p = @params[:domain].to_s.strip.sub(/\A@/, "").presence)
        values << p
      end
      (@parsed.filters[:domain] || []).each { |v| values << v.to_s.sub(/\A@/, "") }

      values.reduce(rel) do |r, v|
        r.where("from_address ILIKE ?", "%@#{sanitize_like(v)}%")
      end
    end

    def filter_recipient(rel)
      values = @parsed.filters[:to] || []
      values.reduce(rel) do |r, v|
        like = "%#{sanitize_like(v)}%"
        r.where("to_address ILIKE :q OR cc_address ILIKE :q", q: like)
      end
    end

    def filter_subject(rel)
      values = @parsed.filters[:subject] || []
      values.reduce(rel) do |r, v|
        r.where("subject ILIKE ?", "%#{sanitize_like(v)}%")
      end
    end

    def filter_dates(rel)
      # Params-sourced dates
      if (from = parse_date(@params[:date_from]))
        rel = rel.where("received_at >= ?", from.beginning_of_day)
      end
      if (to = parse_date(@params[:date_to]))
        rel = rel.where("received_at <= ?", to.end_of_day)
      end
      # Parsed modifier dates (AND semantics — further restrict)
      if (from_str = @parsed.filters[:date_from])
        if (from = parse_date(from_str))
          rel = rel.where("received_at >= ?", from.beginning_of_day)
        end
      end
      if (to_str = @parsed.filters[:date_to])
        if (to = parse_date(to_str))
          rel = rel.where("received_at <= ?", to.end_of_day)
        end
      end
      rel
    end

    def filter_attachment(rel)
      active = @params[:has_attachment].to_s == "1" || @parsed.filters[:has_attachment]
      active ? rel.where(has_attachment: true) : rel
    end

    def filter_unread(rel)
      active = @params[:unread].to_s == "1" || @parsed.filters[:unread]
      active ? rel.where(read: false) : rel
    end

    def filter_read(rel)
      @parsed.filters[:read] ? rel.where(read: true) : rel
    end

    def filter_pinned(rel)
      @parsed.filters[:pinned] ? rel.where.not(pinned_at: nil) : rel
    end

    def filter_category(rel)
      values = [ @params[:category].to_s.strip.presence, *(@parsed.filters[:category] || []) ].compact
      values.reduce(rel) do |r, v|
        r.where(category: v)
      end
    end

    def filter_priority(rel)
      values = []
      if (p = @params[:priority].to_s.strip.presence) && EmailMessage.ai_priorities.key?(p)
        values << p
      end
      (@parsed.filters[:priority] || []).each do |v|
        values << v if EmailMessage.ai_priorities.key?(v)
      end
      values.reduce(rel) do |r, v|
        r.where(ai_priority: v)
      end
    end

    def filter_account_query(rel)
      values = @parsed.filters[:account] || []
      values.reduce(rel) do |r, v|
        ids = @user.readable_email_accounts
                   .where("email_address ILIKE ?", "%#{sanitize_like(v)}%")
                   .ids
        ids.any? ? r.where(email_account_id: ids) : r.none
      end
    end

    # --- Meaning mode ---

    # Hash{EmailMessage.id => raw_score} from the vector index, permission-filtered.
    #   nil => index empty or search errored (caller falls back to keyword-only)
    #   {}  => searched fine, nothing matched
    def semantic_scores
      return nil unless SearchRecord.where(workspace_id: @user.workspace_id).exists?

      raw = SearchService.search(
        query,
        workspace:  @user.workspace,
        filters:    search_service_filters,
        options:    { limit: RESULT_LIMIT, enable_temporal_scoring: false }
      )
      return {} if raw.blank?

      allowed = @user.readable_email_accounts.ids.map(&:to_s).to_set
      raw.each_with_object({}) do |r, h|
        sr = r.search_record
        next unless sr && r.searchable_type == "EmailMessage"
        next unless allowed.include?(sr.filter_data["email_account_id"].to_s)
        h[r.searchable_id] = r.score
      end
    rescue => e
      Rails.logger.warn("[Emails::Search] meaning-mode search failed, falling back to keyword: #{e.message}")
      nil
    end

    # Map our params onto the keys SearchService#apply_filters understands, to
    # pre-narrow the vector candidate pool. Correctness never depends on these (the
    # SQL safety net in #results re-applies every filter); they only improve recall.
    def search_service_filters
      filters      = { searchable_type: "EmailMessage" }
      account_ids  = id_array(@params[:account_ids])
      tag_names    = Tag.where(id: id_array(@params[:tag_ids])).pluck(:name)

      filters[:provider_folder_ids] = @folder_ids if @folder_ids.present?
      filters[:account_ids]         = account_ids if account_ids.any?
      filters[:tags]                = tag_names if tag_names.any?

      # Prefer parsed modifier values when no panel param is set.
      sender_val = @params[:sender].presence || @parsed.filters[:sender]&.first
      filters[:from_address] = sender_val if sender_val.present?

      domain_val = @params[:domain].to_s.strip.sub(/\A@/, "").presence ||
                   @parsed.filters[:domain]&.first
      filters[:sender_domain] = domain_val.downcase if domain_val.present?

      if @params[:date_from].present?
        filters[:date_from] = parse_date(@params[:date_from])&.beginning_of_day
      elsif (df = @parsed.filters[:date_from])
        filters[:date_from] = parse_date(df)&.beginning_of_day
      end

      if @params[:date_to].present?
        filters[:date_to] = parse_date(@params[:date_to])&.end_of_day
      elsif (dt = @parsed.filters[:date_to])
        filters[:date_to] = parse_date(dt)&.end_of_day
      end

      cat = @params[:category].presence || @parsed.filters[:category]&.first
      filters[:category] = cat if cat.present?

      if @params[:unread].to_s == "1" || @parsed.filters[:unread]
        filters[:unread] = "true"
      end

      if @params[:has_attachment].to_s == "1" || @parsed.filters[:has_attachment]
        filters[:has_attachments] = "true"
      end

      pri = @params[:priority].to_s.strip.presence || @parsed.filters[:priority]&.first
      if pri.present? && EmailMessage.ai_priorities.key?(pri)
        filters[:priority] = pri
      end

      filters
    end

    # --- helpers ---

    UUID_RE = /\A[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\z/i

    # Record-id params arrive as uuid strings. Keep only well-formed uuids so a
    # malformed value can't reach a uuid column (which would raise) — and so an
    # all-garbage list yields [] (filter treated as absent), never a silent leak.
    def id_array(value)
      Array(value).filter_map { |v| s = v.to_s.strip.downcase; s if s.match?(UUID_RE) }
    end

    def sanitize_like(str)
      EmailMessage.sanitize_sql_like(str.to_s)
    end

    def parse_date(value)
      return nil if value.blank?
      Date.parse(value.to_s.tr("/", "-"))
    rescue ArgumentError, TypeError
      nil
    end
  end
end
