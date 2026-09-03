# frozen_string_literal: true

# People (Rethink Stage 2): sender types + the message channel dimension.
#
# `sender_kind` classifies a Contact as a person (a human you converse with) or a
# service (machine/bulk mail — newsletters, receipts, notifications, alerts).
# `sender_kind_source` records how the verdict was reached: "heuristic"
# (Contacts::SenderKind) or "taught" (the user corrected it — never overridden).
# `stream_kind` caches which stream a service's mail belongs to (billing /
# notifications / newsletters / social / updates), set alongside sender_kind.
#
# `channel` on messages is the multi-channel seam: every message is "email" today,
# so a second channel later is purely additive (the conversation renders a channel
# chip from it). Backfilled non-null with a default so the column is safe from day
# one.
class AddSenderKindAndChannel < ActiveRecord::Migration[8.1]
  def change
    add_column :contacts, :sender_kind, :integer, default: 0, null: false
    add_column :contacts, :sender_kind_source, :string
    add_column :contacts, :stream_kind, :string
    add_index  :contacts, :sender_kind

    add_column :email_messages, :channel, :string, default: "email", null: false
    add_index  :email_messages, :channel
  end
end
