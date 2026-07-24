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

      # Cross-source + cross-kind soft-dedup pipeline. Only runs for new rows — a
      # persisted pending row re-processing on a re-ingest keeps today's behavior
      # (reassign + save). The pipeline is fail-open: any unexpected path falls
      # through to insert rather than silently dropping a real commitment.
      if !reminder.persisted?
        # 1. Deterministic sibling from another source or a different reminder_type.
        if (sibling = cross_source_sibling(reminder_type, due_at, all_day, item))
          if sibling.dismissed?
            # The user already waved this commitment off — a second email must not
            # resurrect the card.
            Rails.logger.info("[Reminders::Builder] suppressed by dismissed sibling #{sibling.id} for '#{item['title']}'")
            return nil
          end
          return sibling
        end

        # 2. Deterministic calendar check (timed candidates only): a workspace event
        # already scheduled at exactly this instant means the commitment is already
        # on the calendar — don't stage a duplicate reminder card.
        if !all_day
          if CalendarEvent.joins(calendar: :calendar_account)
                          .where(calendar_accounts: { workspace_id: @workspace.id })
                          .visible
                          .exists?(start_at: due_at)
            Rails.logger.info("[Reminders::Builder] suppressed by calendar event at #{due_at.iso8601} for '#{item['title']}'")
            return nil
          end
        end

        # 3. Novelty gate: ask the LLM whether this candidate duplicates a temporal
        # neighbor. Skip when no neighbors exist (no matcher instantiation, no cost).
        neighbors = Commitments::Neighbors.around(workspace: @workspace, due_at: due_at)
        unless neighbors.empty?
          matcher = Ai::CommitmentMatcher.new(
            workspace:  @workspace,
            candidate:  candidate_for(item, due_at, all_day),
            neighbors:  neighbors
          )
          matched = matcher.match

          if matched
            if matched.is_a?(Reminder)
              if matched.dismissed?
                Rails.logger.info("[Reminders::Builder] suppressed by dismissed matched Reminder##{matched.id}")
                return nil
              end
              return matched
            else
              # Matched a Task or CalendarEvent — existing commitment wins across kinds.
              Rails.logger.info("[Reminders::Builder] suppressed by existing #{matched.class}##{matched.id}")
              return nil
            end
          else
            # No match (or check failed) — insert, with the verdict recorded on the item
            # so the stored extracted_data explains why dedup passed.
            verdict = matcher.failed? ? "check_failed" : "no_match"
            item = item.merge("_novelty" => { "neighbors" => neighbors.size, "verdict" => verdict })
          end
        end
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
      # Publish only when this run actually created the row — a pending row re-saved on
      # re-processing must not re-publish and spam /activity.
      if reminder.previously_new_record?
        Events.publish("reminder.created", subject: reminder, actor: nil, payload: { "title" => reminder.title, "due_at" => reminder.due_at&.iso8601 })
      end
      reminder
    rescue ActiveRecord::RecordNotUnique
      # Concurrent build of the same fingerprint — adopt the row that won.
      Reminder.find_by(extraction_fingerprint: fingerprint)
    rescue ActiveRecord::RecordInvalid => e
      Rails.logger.error("[Reminders::Builder] invalid reminder for #{@source.class}##{@source.id}: #{e.message}")
      nil
    end

    # A reminder for the same commitment already staged from a DIFFERENT source: the invoice
    # email vs. its PDF, or (the case this guards) an airline's booking-confirmation and
    # ticket emails, which arrive seconds apart and each extract the same flight. Scoped to
    # same workspace + type + due date (ALL statuses including dismissed — the caller
    # suppresses instead of adopting when the sibling is dismissed), then matched in order of
    # signal strength:
    #   1. exact amount (bills/invoices);
    #   2. exact timestamp for a TIMED event — two same-type reminders at the same minute of
    #      the same day are the same commitment (all-day reminders share a 9am default, so
    #      they're excluded here and fall through to the title match);
    #   3. date-stripped title, so the model appending "on 2026-12-20" to one of two
    #      otherwise-identical titles doesn't split them into separate reminders.
    #   4. type-agnostic title pass: "deadline" vs "event" labelling of the same commitment
    #      still matches when the typed passes all miss (no reminder_type filter, no amount).
    # nil means genuinely no sibling.
    def cross_source_sibling(reminder_type, due_at, all_day, item)
      scope = Reminder.where(workspace: @workspace, reminder_type: reminder_type)
                      .where("due_at::date = ?::date", due_at)

      amount = parse_amount(item["amount_cents"])
      return scope.find_by(amount_cents: amount) if amount

      unless all_day
        timed_match = scope.find_by(all_day: false, due_at: due_at)
        return timed_match if timed_match
      end

      title_key = dedup_title_key(item["title"])
      return nil if title_key.blank?

      typed_match = scope.where(amount_cents: nil).detect { |r| dedup_title_key(r.title) == title_key }
      return typed_match if typed_match

      # Type-agnostic fallback: the same commitment may have been staged under a
      # different reminder_type (e.g. "deadline" from one email, "event" from another).
      # Skip the amount filter here — only nil-amount rows (non-monetary reminders).
      Reminder.where(workspace: @workspace)
              .where("due_at::date = ?::date", due_at)
              .where(amount_cents: nil)
              .detect { |r| dedup_title_key(r.title) == title_key }
    end

    # De-dupe match key: delegates to the shared Commitments::TitleKey module so the
    # same normalisation is used across kinds. Kept private here so existing callers
    # within this file are untouched.
    def dedup_title_key(title)
      Commitments::TitleKey.of(title)
    end

    # Builds the candidate hash passed to Ai::CommitmentMatcher. Mirrors the shape
    # documented in the matcher's constructor: string keys, iso8601 timestamp.
    def candidate_for(item, due_at, all_day)
      {
        "kind"          => "reminder",
        "title"         => item["title"].to_s.strip,
        "due_at"        => due_at.iso8601,
        "timed"         => !all_day,
        "reminder_type" => item["reminder_type"].to_s,
        "amount_cents"  => parse_amount(item["amount_cents"]),
        "currency"      => item["currency"].presence,
        "description"   => item["description"].to_s.first(200).presence
      }
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
