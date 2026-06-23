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

    # Relation for the infinite timeline. The controller paginates this with
    # pagy_countless, then hands the page back to #present.
    def timeline_scope
      @user.feed_items.active.timeline.chronological
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

    # Collapse runs of like items so the timeline reads as clusters, not a wall of
    # identical cards: adjacent tag_suggestion cards fold into one tight,
    # divider-free tag_queue; every other card passes through individually.
    # Returns entries of { type: :card, pair: } or { type: :tag_queue, items: [pairs] }.
    def self.group_timeline(pairs)
      pairs.each_with_object([]) do |pair, grouped|
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
      else type.constantize.all
      end
    end
  end
end
