module Feed
  # Read side of the feed. Turns materialized feed_items into render-ready
  # { item:, subject: } pairs, batch-loading the referenced records (so content
  # is always live) and dropping any whose source says it's no longer valid —
  # the safety net for rows a refresh hasn't reconciled yet.
  class Reader
    STALE_AFTER = 10.minutes

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    # The pinned "needs attention" cluster: urgent items, ranked. Small by
    # construction (high-priority mail, overdue replies, due snoozes), so it's
    # not capped — everything urgent is shown.
    def attention
      @attention ||= present(@user.feed_items.active.attention.ranked.to_a)
    end

    # Relation for the infinite timeline, highest rank first — recency is one
    # input to the rank (Feed::Ranking), not the ordering itself, so a fresh
    # nothing-email can't outrank yesterday's overdue reply (nor a stale
    # follow-up outrank this morning's mail). The controller paginates this
    # with pagy_countless, then hands the page back to #present.
    def timeline_scope
      @user.feed_items.active.timeline.ranked
    end

    # [{ item:, subject: }] for a set of feed_items, subjects batch-loaded with
    # the associations the cards render and still_valid?-filtered.
    def present(items)
      items = items.to_a
      subjects = load_subjects(items)
      sources = {}

      items.filter_map do |item|
        subject = subjects[[ item.subject_type, item.subject_id ]]
        source = (sources[item.kind] ||= build_source(item.kind))
        next unless source
        next unless source.still_valid?(item, subject)
        { item: item, subject: subject }
      end
    end

    # Longest run of same-kind cards the timeline will show back to back.
    MAX_KIND_RUN = 2

    # Shape one timeline page for reading: interleave kinds so no source walls
    # off the page, then collapse adjacent tag_suggestion cards into one tight,
    # divider-free tag_queue; every other card passes through individually.
    # Returns entries of { type: :card, pair: } or { type: :tag_queue, items: [pairs] }.
    # Timeline-only — the attention cluster stays purely rank-ordered.
    def self.group_timeline(pairs)
      interleave(pairs).each_with_object([]) do |pair, grouped|
        if pair[:item].kind == "tag_suggestion"
          if grouped.last&.dig(:type) == :tag_queue
            grouped.last[:items] << pair
          else
            grouped << { type: :tag_queue, items: [ pair ] }
          end
        else
          grouped << { type: :card, pair: pair }
        end
      end
    end

    # Diversity pass over one page, preserving rank order except where a run of
    # MAX_KIND_RUN same-kind cards would continue — then the next different-kind
    # card is pulled forward (an all-one-kind page passes through unchanged).
    # tag_suggestion cards sit out and rejoin at the tail: they gravitate there
    # by score anyway, and group_timeline folds them into a single queue row.
    # Page-local by design: each pagy page is shaped independently, so the DB
    # rank order — what pagination pages over — is never contradicted, just
    # locally reshuffled for reading rhythm.
    def self.interleave(pairs)
      cards, tags = pairs.partition { |pair| pair[:item].kind != "tag_suggestion" }

      result = []
      until cards.empty?
        index = 0
        run_kind = result.last&.dig(:item)&.kind
        if run_kind && result.last(MAX_KIND_RUN).count { |p| p[:item].kind == run_kind } >= MAX_KIND_RUN
          index = cards.index { |pair| pair[:item].kind != run_kind } || 0
        end
        result << cards.delete_at(index)
      end

      result + tags
    end

    # Has generation fallen behind? Drives a debounced background refresh.
    def stale?
      newest = @user.feed_items.maximum(:generated_at)
      newest.nil? || newest < @now - STALE_AFTER
    end

    private

    def build_source(kind)
      Feed::Source.for_kind(kind)&.new(@user, now: @now)
    end

    # Group items by subject type and load each type once, eager-loading the
    # associations the cards (and still_valid? checks) touch — no N+1 per page.
    def load_subjects(items)
      result = {}
      items.group_by(&:subject_type).each do |type, group|
        ids = group.map(&:subject_id)
        subject_relation(type).where(id: ids).each do |record|
          result[[ type, record.id ]] = record
        end
      end
      result
    end

    def subject_relation(type)
      case type
      when "EmailMessage" then EmailMessage.includes(:email_account, :tags, :email_thread, contact: :sender_tags)
      when "Document"     then Document.includes(:classification)
      when "Reminder"     then ::Reminder.includes(:source, :calendar_event)
      when "Task"         then ::Task.includes(:assignees, :tags)
      else type.constantize.all
      end
    end
  end
end
