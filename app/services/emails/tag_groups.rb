# frozen_string_literal: true

module Emails
  # The single source of truth for which inbox threads collapse out of the main
  # list into per-group rows. A group's membership is the UNION of:
  #   1. Tag-based: threads carrying a tag whose group_name matches.
  #   2. Rule-based: threads matched by InboxGroupRule rows for that group.
  #      Rule types: sender (email / @domain), organization (org uuid),
  #      document_type (doc-type uuid), query (structured-filter search string).
  #
  # The four built-in default groups (Notifications / Newsletters & promos /
  # Social / Updates) use tag-based membership only; custom groups can mix tags
  # and rules freely, or be rules-only (no tags required). Additive
  # multi-membership is preserved: the same thread can satisfy multiple groups.
  #
  # A thread is NEVER collapsed when a human clearly cares about it:
  # the owner replied (last_outbound_at), it is pinned, the sender is starred,
  # or any message is classified `important`. The same guards feed both the
  # main-list exclusion and the group rows/counts so the numbers always agree.
  # The caller applies the exclusion on the inbox root only; folder and search
  # views show everything inline.
  class TagGroups
    def initialize(workspace, readable_account_ids)
      @workspace = workspace
      @readable_account_ids = readable_account_ids
    end

    # Guarded EmailThread relation of every grouped thread, for excluding them
    # from the main list. nil when the workspace has neither grouped tags nor
    # group rules.
    def excluded_scope
      return @excluded_scope if defined?(@excluded_scope)

      subqueries = all_tag_subqueries + all_rule_subqueries
      return @excluded_scope = nil if subqueries.empty?

      @excluded_scope = guarded_union(subqueries)
    end

    # Guarded EmailThread relation for one group's drill-in view, or nil when
    # the group name matches neither tags nor rules.
    def group_scope(group_name)
      subqueries = tag_subqueries_for(group_name) + rule_subqueries_for(group_name)
      return nil if subqueries.empty?

      guarded_union(subqueries)
    end

    # The group's display color: first grouped tag's color, or nil for a
    # rules-only group. Callers already render a neutral dot when color is nil.
    def group_color(group_name)
      name = group_name.to_s
      grouped_tags.find { |t| t.group_name == name }&.color
    end

    # Row data for the collapsed group rows: one hash per non-empty group
    # ({ label:, count:, senders:, color: }). Counts are restricted to threads
    # with a message in an inbox folder so the number matches the drill-in view.
    # Includes groups defined only by rules (no tags required).
    def build_groups(inbox_folder_ids)
      all_group_names.filter_map do |group_name|
        subqueries = tag_subqueries_for(group_name) + rule_subqueries_for(group_name)
        next if subqueries.empty?

        scope   = guarded_union(subqueries)
        counted = scope.joins(:email_messages)
        counted = counted.where(email_messages: { provider_folder_id: inbox_folder_ids }) if inbox_folder_ids.present?
        count   = counted.distinct.count
        next if count.zero?

        color = grouped_tags_by_name[group_name]&.first&.color
        { label: group_name, count: count, senders: senders_for(scope), color: color }
      end
    end

    private

    # -------------------------------------------------------------------------
    # Tag-based helpers (unchanged semantics)
    # -------------------------------------------------------------------------

    def grouped_tags
      @grouped_tags ||= Tag.where(workspace_id: @workspace&.id).visible.grouped.by_name.to_a
    end

    def grouped_tags_by_name
      @grouped_tags_by_name ||= grouped_tags.group_by(&:group_name)
    end

    def all_tag_subqueries
      ids = grouped_tags.map(&:id)
      return [] if ids.empty?

      [ threads_with_tags(ids) ]
    end

    def tag_subqueries_for(group_name)
      ids = (grouped_tags_by_name[group_name.to_s] || []).map(&:id)
      ids.any? ? [ threads_with_tags(ids) ] : []
    end

    # email_thread_ids subquery for threads with at least one message carrying
    # one of tag_ids.
    def threads_with_tags(tag_ids)
      EmailMessage.where(email_account_id: @readable_account_ids)
                  .where.not(email_thread_id: nil)
                  .joins(:email_message_tags)
                  .where(email_message_tags: { tag_id: tag_ids })
                  .select(:email_thread_id)
    end

    # -------------------------------------------------------------------------
    # Rule-based helpers
    # -------------------------------------------------------------------------

    def all_rules
      @all_rules ||= InboxGroupRule.where(workspace: @workspace).to_a
    end

    def all_rule_subqueries
      all_rules.filter_map { |rule| thread_id_subquery_for_rule(rule) }
    end

    def rule_subqueries_for(group_name)
      all_rules
        .select { |r| r.group_name == group_name.to_s }
        .filter_map { |rule| thread_id_subquery_for_rule(rule) }
    end

    def thread_id_subquery_for_rule(rule)
      case rule.rule_type
      when "sender"       then threads_matching_sender(rule.value)
      when "organization" then threads_matching_organization(rule.value)
      when "document_type" then threads_matching_document_type(rule.value)
      when "query"        then threads_matching_query(rule.value)
      end
    end

    # Sender rule: bare email address (ILIKE match) or @domain prefix.
    def threads_matching_sender(value)
      base = base_messages
      clause =
        if value.start_with?("@")
          domain = value.delete_prefix("@")
          base.where("from_address ILIKE ?", "%@#{sanitize_like(domain)}")
        else
          base.where("from_address ILIKE ?", "%#{sanitize_like(value)}%")
        end
      clause.select(:email_thread_id)
    end

    # Organization rule: follows the org -> organization_membership -> person ->
    # contact -> email_message chain entirely in SQL via JOINs.
    def threads_matching_organization(org_id)
      base_messages
        .joins(contact: { person: :organization_memberships })
        .where(organization_memberships: { organization_id: org_id })
        .select(:email_thread_id)
    end

    # Document-type rule: EXISTS subquery via document_email_messages.
    def threads_matching_document_type(doc_type_id)
      base_messages
        .where(
          "EXISTS (" \
          "SELECT 1 FROM document_email_messages dem " \
          "INNER JOIN documents d ON d.id = dem.document_id " \
          "WHERE dem.email_message_id = email_messages.id " \
          "AND d.document_type_id = ?)",
          doc_type_id
        )
        .select(:email_thread_id)
    end

    # Query rule: parse the stored query string; apply only its structured
    # filters (free text has no ranking context here and is silently skipped).
    # Returns nil when the query has no applicable filters.
    def threads_matching_query(query_string)
      parsed = Emails::SearchQuery.parse(query_string)
      return nil unless parsed.filters?

      filtered = apply_query_filters(base_messages, parsed.filters)
      filtered.select(:email_thread_id)
    end

    # Apply Emails::SearchQuery structured filters to an EmailMessage scope.
    # Intentionally excludes free-text (no ILIKE on body/summary) and the
    # account: modifier (no user object available in this context).
    def apply_query_filters(rel, filters) # rubocop:disable Metrics/MethodLength
      (filters[:sender] || []).each do |v|
        rel = rel.where("from_address ILIKE ?", "%#{sanitize_like(v)}%")
      end

      (filters[:domain] || []).each do |v|
        domain = v.sub(/\A@/, "")
        rel = rel.where("from_address ILIKE ?", "%@#{sanitize_like(domain)}")
      end

      (filters[:to] || []).each do |v|
        like = "%#{sanitize_like(v)}%"
        rel = rel.where("to_address ILIKE :q OR cc_address ILIKE :q", q: like)
      end

      (filters[:subject] || []).each do |v|
        rel = rel.where("subject ILIKE ?", "%#{sanitize_like(v)}%")
      end

      rel = rel.where(has_attachment: true) if filters[:has_attachment]
      rel = rel.where(read: false) if filters[:unread]
      rel = rel.where(read: true) if filters[:read]
      rel = rel.where.not(pinned_at: nil) if filters[:pinned]

      if (from_str = filters[:date_from]) && (from = parse_date(from_str))
        rel = rel.where("received_at >= ?", from.beginning_of_day)
      end
      if (to_str = filters[:date_to]) && (to = parse_date(to_str))
        rel = rel.where("received_at <= ?", to.end_of_day)
      end

      (filters[:category] || []).each { |v| rel = rel.where(category: v) }

      (filters[:priority] || []).each do |v|
        rel = rel.where(ai_priority: v) if EmailMessage.ai_priorities.key?(v)
      end

      (filters[:tag_names] || []).each do |name|
        ids = Tag.where(workspace: @workspace).where("LOWER(name) = ?", name.downcase).ids
        if ids.any?
          rel = rel.where(
            "EXISTS (SELECT 1 FROM email_message_tags emt " \
            "WHERE emt.email_message_id = email_messages.id AND emt.tag_id IN (?))",
            ids
          )
        else
          rel = rel.none
        end
      end

      rel
    end

    # -------------------------------------------------------------------------
    # Guards (the "a human cares about this thread" gates)
    # -------------------------------------------------------------------------

    # Applies all guards to a union of multiple email_thread_id subqueries,
    # returning an EmailThread scope. Each subquery is an EmailMessage relation
    # selecting :email_thread_id; they are OR-combined at the WHERE level so no
    # per-row N+1 occurs.
    def guarded_union(subqueries)
      combined = subqueries.reduce(nil) do |base, sq|
        candidate = EmailThread.where(id: sq)
        base ? base.or(candidate) : candidate
      end

      combined
        .where(email_account_id: @readable_account_ids)
        .where(last_outbound_at: nil)
        .where.not(id: EmailThread.pinned)
        .where.not(id: starred_sender_thread_ids)
        .where.not(id: important_message_thread_ids)
    end

    def starred_sender_thread_ids
      EmailMessage.joins(:contact)
                  .where(email_account_id: @readable_account_ids)
                  .where.not(email_thread_id: nil)
                  .where.not(contacts: { starred_at: nil })
                  .select(:email_thread_id)
    end

    def important_message_thread_ids
      EmailMessage.where(email_account_id: @readable_account_ids)
                  .where.not(email_thread_id: nil)
                  .where(category: "important")
                  .select(:email_thread_id)
    end

    # -------------------------------------------------------------------------
    # Shared helpers
    # -------------------------------------------------------------------------

    # All group names across tags AND rules, sorted for stable row order.
    def all_group_names
      tag_names  = grouped_tags_by_name.keys
      rule_names = all_rules.map(&:group_name).uniq
      (tag_names + rule_names).uniq.sort
    end

    # Base scope for EmailMessage subqueries: account-scoped + has a thread.
    def base_messages
      EmailMessage.where(email_account_id: @readable_account_ids)
                  .where.not(email_thread_id: nil)
    end

    # Up to 3 distinct recent sender addresses for the row's avatar stack.
    def senders_for(thread_scope)
      sender_rows = EmailMessage.where(email_thread_id: thread_scope.select(:id))
                                .order(received_at: :desc)
                                .limit(20)
                                .pluck(:from_address, :contact_id, :email_account_id)
      top_rows = sender_rows.uniq { |row| row[0] }.first(3)
      account_colors = EmailAccount.where(id: top_rows.map { |row| row[2] }.compact.uniq)
                                   .pluck(:id, :color).to_h
      top_rows.map do |address, contact_id, account_id|
        { email: address, contact_id: contact_id, sent: false, account_color: account_colors[account_id] }
      end
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
