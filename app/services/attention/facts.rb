# frozen_string_literal: true

module Attention
  # All the evidence for one counterpart, loaded in bulk by Attention::Signals
  # and consumed by Attention::Scorer. Pure value object (Data.define); no
  # database access.
  Facts = Data.define(
    :inbound_count,
    :addressed_count,
    :replied_count,
    :median_reply_hours,
    :two_way_threads,
    :outbound_threads,
    :opened_count,
    :meetings_count,
    :invoices_count,
    :settled_count,
    :median_settle_delay_days,
    :archived_unread_count,
    :trashed_count,
    :snoozed_count,
    :forwarded_count,
    :tagged_count,
    :feed_acted_count,
    :feed_dismissed_count,
    :skim_archive_count,
    :skim_keep_count,
    :starred,
    :allowed,
    :blocked,
    :sender_kind,
    :relationship_type,
    :urgency_level,
    :taught,
    :last_activity_at
  ) do
    # Blank facts with all counts zeroed, booleans false, optionals nil.
    def self.blank(**overrides)
      new(
        inbound_count: 0,
        addressed_count: 0,
        replied_count: 0,
        median_reply_hours: nil,
        two_way_threads: 0,
        outbound_threads: 0,
        opened_count: 0,
        meetings_count: 0,
        invoices_count: 0,
        settled_count: 0,
        median_settle_delay_days: nil,
        archived_unread_count: 0,
        trashed_count: 0,
        snoozed_count: 0,
        forwarded_count: 0,
        tagged_count: 0,
        feed_acted_count: 0,
        feed_dismissed_count: 0,
        skim_archive_count: 0,
        skim_keep_count: 0,
        starred: false,
        allowed: false,
        blocked: false,
        sender_kind: nil,
        relationship_type: nil,
        urgency_level: nil,
        taught: nil,
        last_activity_at: nil,
        **overrides
      )
    end

    # Serializable representation for storage in `evidence`. Times become iso8601
    # strings; floats are rounded to 2 decimal places.
    def to_h_for_evidence
      to_h.transform_values do |v|
        case v
        when Time, DateTime then v.iso8601
        when Float then v.round(2)
        else v
        end
      end
    end
  end
end
