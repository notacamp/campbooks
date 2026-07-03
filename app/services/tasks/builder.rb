module Tasks
  # Materializes raw extractor items into Task rows in the `suggested` state, ready
  # for the user to triage. Idempotent: keyed on extraction_fingerprint
  # (fingerprint_source + normalized title) so re-processing never duplicates, and
  # a task the user already triaged (anything past `suggested`) is never
  # overwritten. `fingerprint_source` defaults to the source itself; email
  # extraction passes the THREAD so the same ask restated across a conversation's
  # replies collapses into one task instead of one per message.
  #
  # Unlike Reminders::Builder, a past-due date is KEPT — an overdue action still
  # needs doing. The model's after_create_commit publishes task.created.
  class Builder
    MIN_CONFIDENCE = 0.5    # safety net; the extractor already drops below this
    DEFAULT_HOUR = 9        # tasks with a date but no time get a 9am local slot

    def self.call(workspace:, source:, raw_items:, anchor_tz: Time.zone, fingerprint_source: nil)
      new(workspace: workspace, source: source, anchor_tz: anchor_tz,
          fingerprint_source: fingerprint_source).call(raw_items)
    end

    def initialize(workspace:, source:, anchor_tz: Time.zone, fingerprint_source: nil)
      @workspace = workspace
      @source = source
      @anchor_tz = anchor_tz || Time.zone
      @fingerprint_source = fingerprint_source || source
    end

    def call(raw_items)
      Array(raw_items).filter_map { |item| build_one(item) }
    end

    private

    def build_one(item)
      return nil unless item.is_a?(Hash)
      return nil if item["title"].blank?
      return nil if item["confidence"].to_f < MIN_CONFIDENCE

      title = item["title"].to_s.strip.first(255)
      fingerprint = Task.fingerprint_for(
        source_type: @fingerprint_source.class.name, source_id: @fingerprint_source.id, title: title
      )

      task = Task.find_or_initialize_by(extraction_fingerprint: fingerprint)
      # Never overwrite a task the user already triaged or edited.
      return task if task.persisted? && !task.suggested?

      task.assign_attributes(
        workspace:      @workspace,
        source:         @source,
        title:          title,
        description:    item["description"].presence,
        status:         :suggested,
        priority:       priority_for(item["priority"]),
        due_at:         assemble_due_at(item["due_date"], item["due_time"]),
        ai_suggested:   true,
        confidence:     item["confidence"].to_f.clamp(0.0, 1.0),
        justification:  item["justification"].presence,
        extracted_data: item
      )
      task.save!
      task
    rescue ActiveRecord::RecordNotUnique
      # Concurrent build of the same fingerprint — adopt the row that won.
      Task.find_by(extraction_fingerprint: fingerprint)
    rescue ActiveRecord::RecordInvalid => e
      Rails.logger.error("[Tasks::Builder] invalid task for #{@source.class}##{@source.id}: #{e.message}")
      nil
    end

    def priority_for(val)
      key = val.to_s.downcase
      Task.priorities.key?(key) ? key : "normal"
    end

    def assemble_due_at(date_str, time_str)
      date = parse_date(date_str)
      return nil unless date

      if time_str.to_s.match?(/\A\d{1,2}:\d{2}\z/)
        h, m = time_str.split(":").map(&:to_i)
        @anchor_tz.local(date.year, date.month, date.day, h, m)
      else
        @anchor_tz.local(date.year, date.month, date.day, DEFAULT_HOUR, 0)
      end
    end

    def parse_date(str)
      return nil if str.blank?

      Date.iso8601(str.to_s)
    rescue ArgumentError, TypeError
      Date.parse(str.to_s) rescue nil
    end
  end
end
