# frozen_string_literal: true

module Documents
  # Single param→scope object for the Files area. Understands both the UI
  # filter-panel params and the modifier hash produced by Documents::SearchQuery.
  #
  # Usage:
  #   filters = Documents::Filters.from_params(params)
  #   filters.merge_query(search_query.filters)
  #   filters.apply(scope, workspace: ws, user: u)
  #
  # Panel state and query-modifier state are tracked SEPARATELY: the public
  # readers, `to_h`, `any?` and `active_count` reflect the panel only (modifier
  # constraints are already visible in the query text — they get no chips, no
  # badge count, and never leak into hidden fields or pagination links, where
  # they'd stick around after the user edits the query). `apply` enforces the
  # union of both. Round-trip: Filters.from_params(filters.to_h) yields the same
  # panel state.
  class Filters
    attr_reader :type_ids, :categories, :review_status, :ai_status, :sources,
                :starred, :date_from, :date_to, :amount_min_cents, :amount_max_cents,
                :entities, :numbers, :expense_categories, :folder_id

    def self.from_params(params)
      new(params)
    end

    def initialize(params = {})
      # Accept ActionController::Parameters or plain hashes.
      p = if params.respond_to?(:to_unsafe_h)
        params.to_unsafe_h.with_indifferent_access
      else
        (params || {}).with_indifferent_access
      end

      # Type: array of ids (UUIDs) from `type[]` multi-select.
      @type_ids = Array(p[:type]).flatten.map(&:to_s).reject(&:blank?)

      # Category: single string or array (the panel uses a single select today).
      @categories = Array(p[:category]).flatten.reject(&:blank?).map(&:to_s)

      # Single-value enum filters.
      @review_status = p[:review_status].presence&.to_s
      @ai_status     = p[:ai_status].presence&.to_s

      # Source: array (panel currently uses single select; accepts both).
      @sources = Array(p[:source]).flatten.reject(&:blank?).map(&:to_s)

      # Starred toggle — truthy for "1" or "true".
      @starred = p[:starred].in?([ "1", "true", true ])

      # Date range from explicit date_from/date_to inputs.
      @date_from = parse_date(p[:date_from])
      @date_to   = parse_date(p[:date_to])

      # Legacy month param: "YYYY-MM" → expands to first/last day of that month
      # when neither date_from nor date_to is already set.
      if @date_from.nil? && @date_to.nil? && (month_val = p[:month].presence)
        if (pivot = parse_month(month_val))
          @date_from = pivot.beginning_of_month
          @date_to   = pivot.end_of_month
        end
      end

      # Amount range: panel sends EUR string; convert to cents.
      @amount_min_cents = parse_amount_param(p[:amount_min])
      @amount_max_cents = parse_amount_param(p[:amount_max])

      # Text-style filters (single or array; panel uses single inputs today).
      @entities           = Array(p[:entity]).flatten.reject(&:blank?).map(&:to_s)
      @numbers            = Array(p[:number]).flatten.reject(&:blank?).map(&:to_s)
      @expense_categories = Array(p[:expense_category]).flatten.reject(&:blank?).map(&:to_s)

      # Folder by id (panel/mobile picker); a folder NAME only arrives via merge_query.
      @folder_id = p[:folder_id].presence&.to_s

      reset_query_state
    end

    # Merge in the parsed modifier filters from Documents::SearchQuery#filters.
    # Stored apart from the panel state (see class comment); `apply` unions the
    # two, with modifiers winning for single-value dimensions. Returns self.
    def merge_query(query_filters)
      return self if query_filters.blank?

      qf = query_filters.with_indifferent_access

      @q_type_names         = Array(qf[:type_names]).flatten.reject(&:blank?).uniq
      @q_categories         = Array(qf[:categories]).flatten.reject(&:blank?).uniq
      @q_sources            = Array(qf[:sources]).flatten.reject(&:blank?).uniq
      @q_entities           = Array(qf[:entities]).flatten.reject(&:blank?).uniq
      @q_numbers            = Array(qf[:numbers]).flatten.reject(&:blank?).uniq
      @q_expense_categories = Array(qf[:expense_categories]).flatten.reject(&:blank?).uniq

      @q_review_status = qf[:review_status].presence&.to_s
      @q_ai_status     = qf[:ai_status].presence&.to_s
      @q_starred       = qf[:starred] ? true : false
      @q_date_from     = coerce_date(qf[:date_from]) if qf[:date_from].present?
      @q_date_to       = coerce_date(qf[:date_to])   if qf[:date_to].present?
      @q_amount_min_cents = qf[:amount_min_cents].to_i if qf[:amount_min_cents].present?
      @q_amount_max_cents = qf[:amount_max_cents].to_i if qf[:amount_max_cents].present?
      @q_folder_name      = qf[:folder_name].presence&.to_s

      self
    end

    # Chain the active filter dimensions (panel ∪ query) onto +scope+ and return
    # the narrowed relation.
    #
    # Type names from modifiers are resolved to ids via workspace.document_types.
    # Folder name is resolved via workspace.mail_folders.accessible_to(user) — an
    # unknown name results in scope.none. Pass user: nil to skip folder-name
    # resolution entirely (export path: stored filters only ever carry folder ids).
    def apply(scope, workspace:, user:)
      ids = effective_type_ids(workspace)
      scope = scope.by_type(ids) if ids.any?

      scope = scope.by_category(effective_categories)           if effective_categories.any?
      scope = scope.by_review_status(effective_review_status)   if effective_review_status.present?
      scope = scope.by_ai_status(effective_ai_status)           if effective_ai_status.present?
      scope = scope.by_source(effective_sources)                if effective_sources.any?
      scope = scope.starred_only(true)                          if effective_starred
      scope = scope.document_date_from(effective_date_from)     if effective_date_from.present?
      scope = scope.document_date_to(effective_date_to)         if effective_date_to.present?
      scope = scope.amount_at_least(effective_amount_min_cents) if effective_amount_min_cents.present?
      scope = scope.amount_at_most(effective_amount_max_cents)  if effective_amount_max_cents.present?
      scope = scope.by_entity(effective_entities)               if effective_entities.any?
      scope = scope.by_reference(effective_numbers)             if effective_numbers.any?
      scope = scope.by_expense_category(effective_expense_categories) if effective_expense_categories.any?

      # Folder resolution: name (from modifier) takes precedence over id (panel).
      if @q_folder_name.present? && user
        folder = workspace.mail_folders
          .accessible_to(user)
          .find_by("LOWER(name) = ?", @q_folder_name.downcase)
        return scope.none unless folder
        scope = scope.in_folder(folder.id)
      elsif @folder_id.present?
        scope = scope.in_folder(@folder_id)
      end

      scope
    end

    # True when at least one panel filter is active (excludes modifier-only state).
    def any?
      active_count > 0
    end

    # True when ANY constraint — panel or query modifier — narrows the documents.
    # Drives "no matches" vs first-run empty states.
    def narrowing?
      any? || query_state?
    end

    # Count of active panel-filter selections (like EmailSearchBar#active_count).
    # Array params count per-selection; single params count as 1.
    def active_count
      count  = @type_ids.size
      count += @categories.size
      count += 1 if @review_status.present?
      count += 1 if @ai_status.present?
      count += @sources.size
      count += 1 if @starred
      count += 1 if @date_from.present?
      count += 1 if @date_to.present?
      count += 1 if @amount_min_cents.present?
      count += 1 if @amount_max_cents.present?
      count += @entities.size
      count += @numbers.size
      count += @expense_categories.size
      count += 1 if @folder_id.present?
      count
    end

    # Compact, stringable hash of the PANEL params only (no modifier-derived
    # state — that lives in the query text and rides along as `q`). Designed for
    # hidden-field loops, pagination link merging, and chip removal links.
    #
    # Round-trip guarantee: Filters.from_params(filters.to_h) produces the same
    # active panel-filter set.
    def to_h
      h = {}
      h[:type]             = @type_ids                      if @type_ids.any?
      h[:category]         = @categories                    if @categories.any?
      h[:review_status]    = @review_status                 if @review_status.present?
      h[:ai_status]        = @ai_status                     if @ai_status.present?
      h[:source]           = @sources                       if @sources.any?
      h[:starred]          = "1"                            if @starred
      h[:date_from]        = @date_from.strftime("%Y-%m-%d") if @date_from.present?
      h[:date_to]          = @date_to.strftime("%Y-%m-%d")   if @date_to.present?
      h[:amount_min]       = format_amount(@amount_min_cents) if @amount_min_cents.present?
      h[:amount_max]       = format_amount(@amount_max_cents) if @amount_max_cents.present?
      h[:entity]           = @entities                      if @entities.any?
      h[:number]           = @numbers                       if @numbers.any?
      h[:expense_category] = @expense_categories            if @expense_categories.any?
      h[:folder_id]        = @folder_id                     if @folder_id.present?
      h
    end

    # The FULL merged constraint set (panel ∪ query), materialized to plain
    # params — for persistence where the query text won't be available later
    # (ExportJob re-applies the stored hash long after the request). Type names
    # resolve to ids here; the folder modifier resolves to an id under +user+'s
    # permissions (unknown name → the reserved "none" id, so the stored filter
    # stays as restrictive as the live view that produced it).
    def to_persistable_h(workspace:, user:)
      h = to_h

      ids = effective_type_ids(workspace)
      h[:type] = ids if ids.any?
      h[:category]         = effective_categories         if effective_categories.any?
      h[:review_status]    = effective_review_status      if effective_review_status.present?
      h[:ai_status]        = effective_ai_status          if effective_ai_status.present?
      h[:source]           = effective_sources            if effective_sources.any?
      h[:starred]          = "1"                          if effective_starred
      h[:date_from]        = effective_date_from.strftime("%Y-%m-%d") if effective_date_from.present?
      h[:date_to]          = effective_date_to.strftime("%Y-%m-%d")   if effective_date_to.present?
      h[:amount_min]       = format_amount(effective_amount_min_cents) if effective_amount_min_cents.present?
      h[:amount_max]       = format_amount(effective_amount_max_cents) if effective_amount_max_cents.present?
      h[:entity]           = effective_entities           if effective_entities.any?
      h[:number]           = effective_numbers            if effective_numbers.any?
      h[:expense_category] = effective_expense_categories if effective_expense_categories.any?

      if @q_folder_name.present? && user
        folder = workspace.mail_folders.accessible_to(user)
          .find_by("LOWER(name) = ?", @q_folder_name.downcase)
        h[:folder_id] = folder ? folder.id.to_s : MISSING_FOLDER_ID
      end

      h
    end

    # True when any dimension that only Documents can satisfy is active — panel
    # or modifier. Used by FilesController to exclude internal docs and filed
    # emails from the result set when such a filter is applied (those items have
    # no vendor_name, review_status, etc., so they'd appear confusingly in an
    # otherwise-filtered list). Date range is NOT document-specific (it narrows
    # internal docs/emails by their own timestamps).
    def document_specific?
      @type_ids.any?               || @q_type_names.any?          ||
        @categories.any?           || @q_categories.any?          ||
        @review_status.present?    || @q_review_status.present?   ||
        @ai_status.present?        || @q_ai_status.present?       ||
        @sources.any?              || @q_sources.any?             ||
        @starred                   || @q_starred                  ||
        @amount_min_cents.present? || @q_amount_min_cents.present? ||
        @amount_max_cents.present? || @q_amount_max_cents.present? ||
        @entities.any?             || @q_entities.any?            ||
        @numbers.any?              || @q_numbers.any?             ||
        @expense_categories.any?   || @q_expense_categories.any?  ||
        @folder_id.present?        || @q_folder_name.present?
    end

    # The effective date range (query modifiers win), applicable to internal
    # docs (created_at) and filed emails (received_at). Nil when inactive.
    def date_range
      from = effective_date_from
      to   = effective_date_to
      return nil unless from.present? || to.present?

      start_dt = from ? from.beginning_of_day : Time.at(0).utc
      end_dt   = to   ? to.end_of_day         : Time.current
      start_dt..end_dt
    end

    private

    # A well-formed-but-unassignable folder id: filtering by it matches nothing,
    # mirroring apply's scope.none for an unknown folder name.
    MISSING_FOLDER_ID = "00000000-0000-0000-0000-000000000000"

    def reset_query_state
      @q_type_names         = []
      @q_categories         = []
      @q_sources            = []
      @q_entities           = []
      @q_numbers            = []
      @q_expense_categories = []
      @q_review_status      = nil
      @q_ai_status          = nil
      @q_starred            = false
      @q_date_from          = nil
      @q_date_to            = nil
      @q_amount_min_cents   = nil
      @q_amount_max_cents   = nil
      @q_folder_name        = nil
    end

    def query_state?
      @q_type_names.any? || @q_categories.any? || @q_sources.any? ||
        @q_entities.any? || @q_numbers.any? || @q_expense_categories.any? ||
        @q_review_status.present? || @q_ai_status.present? || @q_starred ||
        @q_date_from.present? || @q_date_to.present? ||
        @q_amount_min_cents.present? || @q_amount_max_cents.present? ||
        @q_folder_name.present?
    end

    # ── Effective (panel ∪ query) values ─────────────────────────────────────

    # Panel ids plus modifier type names resolved through the workspace's types.
    def effective_type_ids(workspace)
      ids = @type_ids.dup
      if @q_type_names.any?
        names_lower = @q_type_names.map(&:downcase)
        ids += workspace.document_types.where("LOWER(name) IN (?)", names_lower).pluck(:id).map(&:to_s)
      end
      ids.uniq
    end

    def effective_categories         = (@categories + @q_categories).uniq
    def effective_sources            = (@sources + @q_sources).uniq
    def effective_entities           = (@entities + @q_entities).uniq
    def effective_numbers            = (@numbers + @q_numbers).uniq
    def effective_expense_categories = (@expense_categories + @q_expense_categories).uniq
    def effective_review_status      = @q_review_status || @review_status
    def effective_ai_status          = @q_ai_status || @ai_status
    def effective_starred            = @starred || @q_starred
    def effective_date_from          = @q_date_from || @date_from
    def effective_date_to            = @q_date_to || @date_to
    def effective_amount_min_cents   = @q_amount_min_cents || @amount_min_cents
    def effective_amount_max_cents   = @q_amount_max_cents || @amount_max_cents

    # ── Parsing helpers ───────────────────────────────────────────────────────

    def parse_date(val)
      return nil if val.blank?
      Date.strptime(val.to_s, "%Y-%m-%d")
    rescue ArgumentError
      nil
    end

    def parse_month(val)
      Date.strptime("#{val}-01", "%Y-%m-%d")
    rescue ArgumentError
      nil
    end

    # Parse a EUR amount string (from a number input) to integer cents.
    # Lenient: bad input returns nil (filter is silently ignored).
    def parse_amount_param(val)
      return nil if val.blank?
      s = val.to_s.strip.gsub(/[€$\s]/, "")
      return nil if s.blank?
      (BigDecimal(s) * 100).round.to_i
    rescue ArgumentError, TypeError
      nil
    end

    def coerce_date(val)
      return val if val.is_a?(Date)
      Date.parse(val.to_s)
    rescue ArgumentError
      nil
    end

    def format_amount(cents)
      return nil unless cents
      (cents / 100.0).to_s
    end
  end
end
