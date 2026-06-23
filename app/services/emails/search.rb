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
  class Search
    # A free-text query runs a relevance search (embedding + keyword) and returns a
    # single bounded, rank-ordered page — HNSW has no stable offset, so relevance
    # results don't paginate. This caps the result/candidate pool.
    RESULT_LIMIT = 100

    # @param folder_ids [Array<String>, nil] pre-resolved provider folder ids.
    #   nil => no folder filter; [] => a folder was chosen but matched nothing.
    def initialize(user:, params:, folder_ids: nil)
      @user = user
      @params = params || {}
      @folder_ids = folder_ids
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
    # embedding similarity with exact keyword matches. Embedding hits lead (by
    # similarity); any literal keyword matches the vector search missed are appended
    # (recent first), so an exact lookup is never lost. When the index is empty or
    # the embedding call fails, semantic_ids is nil and this degrades to keyword
    # only. All filters + permission are re-applied in SQL on the merged id set.
    def results
      ids = ((semantic_ids || []) + keyword_ids).uniq.first(RESULT_LIMIT)
      return [] if ids.empty?

      by_id = apply_structural_filters(base_scope).where(id: ids).index_by(&:id)
      ids.filter_map { |id| by_id[id] }
    end

    # Ids of literal keyword matches (subject/from/ai_summary), already filtered +
    # permission-scoped + recency-ordered (scope), capped to the result pool.
    def keyword_ids
      scope.limit(RESULT_LIMIT).pluck(:id)
    end

    private

    def query
      @params[:q].to_s.strip.presence
    end

    def base_scope
      EmailMessage.accessible_to(@user)
    end

    # --- Structural (SQL) filters, applied in both modes ---

    def apply_structural_filters(rel)
      rel = filter_folder(rel)
      rel = filter_accounts(rel)
      rel = filter_tags(rel)
      rel = filter_sender(rel)
      rel = filter_domain(rel)
      rel = filter_dates(rel)
      rel = filter_attachment(rel)
      rel = filter_unread(rel)
      rel = filter_category(rel)
      filter_priority(rel)
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
      ids = int_array(@params[:account_ids])
      return rel if ids.empty?
      allowed = @user.readable_email_accounts.where(id: ids).ids
      allowed.any? ? rel.where(email_account_id: allowed) : rel.none
    end

    def filter_tags(rel)
      ids = int_array(@params[:tag_ids])
      return rel if ids.empty?

      if @params[:tag_match].to_s == "all"
        # Every selected tag must be present — one EXISTS per tag avoids the
        # cartesian blow-up of joining the tag table N times.
        ids.reduce(rel) do |scoped, tag_id|
          scoped.where(
            "EXISTS (SELECT 1 FROM email_message_tags emt " \
            "WHERE emt.email_message_id = email_messages.id AND emt.tag_id = ?)",
            tag_id
          )
        end
      else
        rel.joins(:email_message_tags).where(email_message_tags: { tag_id: ids }).distinct
      end
    end

    def filter_sender(rel)
      value = @params[:sender].to_s.strip.presence
      return rel unless value
      rel.where("from_address ILIKE ?", "%#{sanitize_like(value)}%")
    end

    def filter_domain(rel)
      value = @params[:domain].to_s.strip.sub(/\A@/, "").presence
      return rel unless value
      rel.where("from_address ILIKE ?", "%@#{sanitize_like(value)}%")
    end

    def filter_dates(rel)
      if (from = parse_date(@params[:date_from]))
        rel = rel.where("received_at >= ?", from.beginning_of_day)
      end
      if (to = parse_date(@params[:date_to]))
        rel = rel.where("received_at <= ?", to.end_of_day)
      end
      rel
    end

    def filter_attachment(rel)
      @params[:has_attachment].to_s == "1" ? rel.where(has_attachment: true) : rel
    end

    def filter_unread(rel)
      @params[:unread].to_s == "1" ? rel.where(read: false) : rel
    end

    def filter_category(rel)
      value = @params[:category].to_s.strip.presence
      value ? rel.where(category: value) : rel
    end

    def filter_priority(rel)
      value = @params[:priority].to_s.strip.presence
      return rel unless value && EmailMessage.ai_priorities.key?(value)
      rel.where(ai_priority: value)
    end

    # --- Meaning mode ---

    # Ranked EmailMessage ids from the vector index, permission-filtered by account.
    #   nil => index empty or search errored (caller falls back to keyword scope)
    #   []  => searched fine, nothing matched
    def semantic_ids
      return nil unless SearchRecord.where(workspace_id: @user.workspace_id).exists?

      raw = SearchService.search(
        query,
        workspace: @user.workspace,
        filters: search_service_filters,
        options: { limit: RESULT_LIMIT }
      )
      return [] if raw.blank?

      allowed = @user.readable_email_accounts.ids.to_set
      raw.filter_map do |r|
        sr = r.search_record
        next unless sr && r.searchable_type == "EmailMessage"
        next unless allowed.include?(sr.filter_data["email_account_id"].to_i)
        r.searchable_id
      end
    rescue => e
      Rails.logger.warn("[Emails::Search] meaning-mode search failed, falling back to keyword: #{e.message}")
      nil
    end

    # Map our params onto the keys SearchService#apply_filters understands, to
    # pre-narrow the vector candidate pool. Correctness never depends on these (the
    # SQL safety net in #results re-applies every filter); they only improve recall.
    def search_service_filters
      filters = { searchable_type: "EmailMessage" }
      account_ids = int_array(@params[:account_ids])
      tag_names = Tag.where(id: int_array(@params[:tag_ids])).pluck(:name)

      filters[:provider_folder_ids] = @folder_ids if @folder_ids.present?
      filters[:account_ids] = account_ids if account_ids.any?
      filters[:tags] = tag_names if tag_names.any?
      filters[:from_address] = @params[:sender] if @params[:sender].present?
      filters[:sender_domain] = @params[:domain].to_s.strip.sub(/\A@/, "").downcase if @params[:domain].present?
      filters[:date_from] = parse_date(@params[:date_from])&.beginning_of_day if @params[:date_from].present?
      filters[:date_to] = parse_date(@params[:date_to])&.end_of_day if @params[:date_to].present?
      filters[:category] = @params[:category] if @params[:category].present?
      filters[:unread] = "true" if @params[:unread].to_s == "1"
      filters[:has_attachments] = "true" if @params[:has_attachment].to_s == "1"
      if @params[:priority].present? && EmailMessage.ai_priorities.key?(@params[:priority].to_s)
        filters[:priority] = @params[:priority]
      end
      filters
    end

    # --- helpers ---

    def int_array(value)
      Array(value).filter_map { |v| Integer(v.to_s.strip, exception: false) }
    end

    def sanitize_like(str)
      EmailMessage.sanitize_sql_like(str.to_s)
    end

    def parse_date(value)
      return nil if value.blank?
      Date.parse(value.to_s)
    rescue ArgumentError, TypeError
      nil
    end
  end
end
