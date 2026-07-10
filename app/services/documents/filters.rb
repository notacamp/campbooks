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
  # `to_h` returns a compact, stringable hash of UI params only (no modifier
  # state) — safe to embed as hidden fields, pagination links, and chip URLs.
  # Round-trip: Filters.from_params(filters.to_h) yields the same active filters.
  class Filters
    attr_reader :type_ids, :categories, :review_status, :ai_status, :sources,
                :starred, :date_from, :date_to, :amount_min_cents, :amount_max_cents,
                :entities, :numbers, :expense_categories, :folder_id, :folder_name

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
      @entities          = Array(p[:entity]).flatten.reject(&:blank?).map(&:to_s)
      @numbers           = Array(p[:number]).flatten.reject(&:blank?).map(&:to_s)
      @expense_categories = Array(p[:expense_category]).flatten.reject(&:blank?).map(&:to_s)

      # Folder by id (panel/mobile picker); name comes from modifier merging.
      @folder_id   = p[:folder_id].presence&.to_s
      @folder_name = nil # set by merge_query only

      # Modifier-derived type names (not UI params) — tracked separately so
      # active_count/to_h don't include them.
      @type_names_from_query = []
    end

    # Merge in the parsed modifier filters from Documents::SearchQuery#filters.
    # Convention: arrays union (deduped); singles — modifier wins over panel value.
    # Returns self so it can be chained.
    def merge_query(query_filters)
      return self if query_filters.blank?

      qf = query_filters.with_indifferent_access

      # Type names from the query (e.g. type:receipt) — resolved to ids in apply.
      @type_names_from_query = Array(qf[:type_names]).flatten.reject(&:blank?).uniq

      # Union of categories.
      @categories = (@categories + Array(qf[:categories]).flatten.reject(&:blank?)).uniq

      # Singles: modifier wins.
      @review_status = qf[:review_status].to_s if qf[:review_status].present?
      @ai_status     = qf[:ai_status].to_s     if qf[:ai_status].present?

      # Union of sources.
      @sources = (@sources + Array(qf[:sources]).flatten.reject(&:blank?)).uniq

      # Starred: once set it stays set.
      @starred = true if qf[:starred]

      # Date range: modifier wins over panel value.
      if qf[:date_from].present?
        @date_from = coerce_date(qf[:date_from])
      end
      if qf[:date_to].present?
        @date_to = coerce_date(qf[:date_to])
      end

      # Amount range: modifier wins.
      @amount_min_cents = qf[:amount_min_cents].to_i if qf[:amount_min_cents].present?
      @amount_max_cents = qf[:amount_max_cents].to_i if qf[:amount_max_cents].present?

      # Union of entity/number/expense arrays.
      @entities           = (@entities + Array(qf[:entities]).flatten.reject(&:blank?)).uniq
      @numbers            = (@numbers  + Array(qf[:numbers]).flatten.reject(&:blank?)).uniq
      @expense_categories = (@expense_categories + Array(qf[:expense_categories]).flatten.reject(&:blank?)).uniq

      # Folder name from modifier (single, last wins).
      @folder_name = qf[:folder_name].to_s if qf[:folder_name].present?

      self
    end

    # Chain the active filter dimensions onto +scope+ and return the narrowed relation.
    #
    # Type names from modifiers are resolved to ids via workspace.document_types.
    # Folder name is resolved via workspace.mail_folders.accessible_to(user) — an
    # unknown name results in scope.none. Pass user: nil to skip folder-name
    # resolution entirely (export path: no per-user permission check needed there).
    def apply(scope, workspace:, user:)
      # Combine panel type_ids with query type_names resolved to ids.
      combined_type_ids = @type_ids.dup
      if @type_names_from_query.any?
        names_lower = @type_names_from_query.map(&:downcase)
        name_ids = workspace.document_types
          .where("LOWER(name) IN (?)", names_lower)
          .pluck(:id)
        combined_type_ids = (combined_type_ids + name_ids).uniq
      end
      scope = scope.by_type(combined_type_ids) if combined_type_ids.any?

      scope = scope.by_category(@categories)        if @categories.any?
      scope = scope.by_review_status(@review_status) if @review_status.present?
      scope = scope.by_ai_status(@ai_status)         if @ai_status.present?
      scope = scope.by_source(@sources)              if @sources.any?
      scope = scope.starred_only(@starred)           if @starred
      scope = scope.document_date_from(@date_from)   if @date_from.present?
      scope = scope.document_date_to(@date_to)       if @date_to.present?
      scope = scope.amount_at_least(@amount_min_cents) if @amount_min_cents.present?
      scope = scope.amount_at_most(@amount_max_cents)  if @amount_max_cents.present?
      scope = scope.by_entity(@entities)             if @entities.any?
      scope = scope.by_reference(@numbers)           if @numbers.any?
      scope = scope.by_expense_category(@expense_categories) if @expense_categories.any?

      # Folder resolution: name (from modifier) takes precedence over id (panel).
      if @folder_name.present? && user
        folder = workspace.mail_folders
          .accessible_to(user)
          .find_by("LOWER(name) = ?", @folder_name.downcase)
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

    # Compact, stringable hash of UI params only (no modifier-derived state).
    # Designed for hidden-field loops, pagination link merging, chip removal links,
    # and export filter persistence.
    #
    # Round-trip guarantee: Filters.from_params(filters.to_h) produces the same
    # active filter set.
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

    # True when any dimension that only Documents can satisfy is active.  Used by
    # FilesController to exclude internal docs and filed emails from the result set
    # when such a filter is applied (those items have no vendor_name, review_status,
    # etc., so they'd appear confusingly in an otherwise-filtered list).
    def document_specific?
      @type_ids.any?                || @type_names_from_query.any? ||
        @categories.any?            || @review_status.present?     ||
        @ai_status.present?         || @sources.any?               ||
        @starred                    || @amount_min_cents.present?   ||
        @amount_max_cents.present?  || @entities.any?              ||
        @numbers.any?               || @expense_categories.any?    ||
        @folder_id.present?         || @folder_name.present?
    end

    # The date range applicable to internal-docs (created_at) and filed emails
    # (received_at).  Returns nil when no date filter is active.
    def date_range
      return nil unless @date_from.present? || @date_to.present?

      start_dt = @date_from ? @date_from.beginning_of_day : Time.at(0).utc
      end_dt   = @date_to   ? @date_to.end_of_day         : Time.current
      start_dt..end_dt
    end

    private

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
