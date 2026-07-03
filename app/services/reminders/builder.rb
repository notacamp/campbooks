module Reminders
  # Materializes raw extractor items into Reminder rows. Idempotent: keyed on
  # extraction_fingerprint so re-processing the same source never duplicates, and a
  # reminder the user already acted on is never overwritten. Assembles due_at in the
  # source's anchor time zone to avoid midnight/zone off-by-one.
  class Builder
    MIN_CONFIDENCE = 0.5    # safety net; the extractor already drops below this
    DEFAULT_HOUR = 9        # all-day reminders get a 9am local slot for the calendar

    def self.call(workspace:, source:, raw_items:, anchor_tz: Time.zone, learning_memory: nil)
      new(workspace: workspace, source: source, anchor_tz: anchor_tz, learning_memory: learning_memory).call(raw_items)
    end

    def initialize(workspace:, source:, anchor_tz: Time.zone, learning_memory: nil)
      @workspace = workspace
      @source = source
      @anchor_tz = anchor_tz || Time.zone
      @learning_memory = learning_memory
    end

    def call(raw_items)
      Array(raw_items).filter_map { |item| build_one(item) }
    end

    private

    def build_one(item)
      return nil unless item.is_a?(Hash)
      return nil if item["confidence"].to_f < MIN_CONFIDENCE

      reminder_type = item["reminder_type"].to_s
      return nil unless Reminder.reminder_types.key?(reminder_type)

      # Learning ConfidenceGuard: when this workspace strongly and specifically dismisses
      # this sender's reminders of this type, don't stage another — the confirm/dismiss
      # history speaks. Sender-specific tiers only (never the broad type/global tier), and
      # never suppress on error (learning must never poison the pipeline).
      signal = learning_signal(reminder_type)
      if suppress?(signal)
        Rails.logger.info("[Reminders::Builder] learning-guard suppressed #{reminder_type} from #{@source.class}##{@source.id}")
        return nil
      end

      due_date = parse_date(item["due_date"])
      return nil unless due_date

      all_day = item["all_day"] == true || item["due_time"].blank?
      due_at  = assemble_due_at(due_date, item["due_time"], all_day)

      # Don't stage reminders for dates that have already passed. A past-due
      # suggestion can't be acted on — RemindersController hides it — so creating
      # one only spams the activity feed with an invisible "reminder.created".
      # "Today" still counts as actionable: floor to the start of the current day,
      # matching the page's own visibility rule (pending where due_at >= today).
      return nil if due_at < Time.current.beginning_of_day

      fingerprint = Reminder.fingerprint_for(
        source_type: @source.class.name, source_id: @source.id,
        reminder_type: reminder_type, due_date: due_date.iso8601
      )

      reminder = Reminder.find_or_initialize_by(extraction_fingerprint: fingerprint)
      # Never overwrite a reminder the user already confirmed/dismissed/snoozed.
      return reminder if reminder.persisted? && !reminder.pending?

      # Cross-source soft-dedup: the same commitment arriving as an email body AND its
      # PDF attachment fingerprints differently (source_id differs), so without this both
      # would surface a card. Adopt the sibling already staged from the other source.
      # NOTE: two extraction jobs racing can both pass this guard (no DB constraint) —
      # accepted until a (workspace, reminder_type, due_at::date, amount_cents) unique
      # index lands; in practice the document job lags the email job by seconds.
      if !reminder.persisted? && (sibling = cross_source_sibling(reminder_type, due_at, item))
        return sibling
      end

      # Provenance: record the matched learning signal alongside the extraction so the
      # UI can later explain "based on N of M past reminders from this sender".
      item = item.merge("_learning" => learning_provenance(signal)) if signal

      reminder.assign_attributes(
        workspace:     @workspace,
        source:        @source,
        reminder_type: reminder_type,
        title:         item["title"].to_s.strip.first(255),
        description:   item["description"].presence,
        due_at:        due_at,
        all_day:       all_day,
        status:        :pending,
        confidence:    item["confidence"].to_f.clamp(0.0, 1.0),
        amount_cents:  parse_amount(item["amount_cents"]),
        currency:      item["currency"].presence,
        justification: item["justification"].presence,
        extracted_data: item
      )
      reminder.save!
      Events.publish("reminder.created", subject: reminder, actor: nil, payload: { "title" => reminder.title, "due_at" => reminder.due_at&.iso8601 })
      reminder
    rescue ActiveRecord::RecordNotUnique
      # Concurrent build of the same fingerprint — adopt the row that won.
      Reminder.find_by(extraction_fingerprint: fingerprint)
    rescue ActiveRecord::RecordInvalid => e
      Rails.logger.error("[Reminders::Builder] invalid reminder for #{@source.class}##{@source.id}: #{e.message}")
      nil
    end

    # A reminder for the same commitment already staged from a DIFFERENT source (e.g. the
    # invoice email vs. its PDF). Same workspace + type + due date, not dismissed; matched
    # on amount when the AI extracted one, else on the normalized title. nil ⇒ no sibling.
    def cross_source_sibling(reminder_type, due_at, item)
      scope = Reminder.where(workspace: @workspace, reminder_type: reminder_type)
                      .where.not(status: :dismissed)
                      .where("due_at::date = ?::date", due_at)
      amount = parse_amount(item["amount_cents"])
      if amount
        scope.find_by(amount_cents: amount)
      else
        title_key = Emails::SubjectNormalizer.key(item["title"].to_s)
        return nil if title_key.blank?
        scope.where(amount_cents: nil).detect { |r| Emails::SubjectNormalizer.key(r.title) == title_key }
      end
    end

    def parse_date(str)
      Date.iso8601(str.to_s)
    rescue ArgumentError, TypeError
      Date.parse(str.to_s) rescue nil
    end

    def assemble_due_at(date, time_str, all_day)
      if !all_day && time_str.to_s.match?(/\A\d{1,2}:\d{2}\z/)
        h, m = time_str.split(":").map(&:to_i)
        @anchor_tz.local(date.year, date.month, date.day, h, m)
      else
        @anchor_tz.local(date.year, date.month, date.day, DEFAULT_HOUR, 0)
      end
    end

    def parse_amount(val)
      return nil if val.blank?
      Integer(val)
    rescue ArgumentError, TypeError
      nil
    end

    # The learned confirm/dismiss consensus for this sender + reminder_type, or nil.
    # Fail-safe: a nil memory or any error means "no signal" (never suppresses).
    def learning_signal(reminder_type)
      return nil unless @learning_memory

      @learning_memory.suggestion(
        contact_id: signals[:contact_id], sender_domain: signals[:sender_domain], reminder_type: reminder_type
      )
    rescue => e
      Rails.logger.warn("[Reminders::Builder] learning_signal failed: #{e.message}")
      nil
    end

    # Derived once per source (email/document), reused across its items.
    def signals
      @signals ||= Learning::EmailSignals.for(@source)
    end

    # Suppress only on a strong, SENDER-SPECIFIC dismissed consensus — a broad
    # type/global "we dismiss these" is too blunt to hard-drop a new sender's reminder.
    def suppress?(signal)
      return false unless signal

      signal.label == "dismissed" && %i[contact domain].include?(signal.source)
    end

    def learning_provenance(signal)
      { "signal" => signal.source.to_s, "label" => signal.label,
        "count" => signal.count, "total" => signal.total }
    end
  end
end
