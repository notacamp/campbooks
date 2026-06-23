class AddReplyTrackingToEmailThreads < ActiveRecord::Migration[8.1]
  # Denormalized reply state + AI follow-up verdict, so Skim and the Feed can hide
  # conversations the owner already answered and resurface only the ones genuinely
  # awaiting a reply.
  #
  # last_outbound_at / last_inbound_at are maintained by EmailProcessJob (and the
  # in-app send paths) as messages are processed. "Holds last word" — the owner had
  # the last say, so the ball is in the other party's court — is then an indexable
  # column check instead of a per-thread query: a message belongs to the owner when
  # its from_address CONTAINS the account address (mirrors EmailProcessJob's
  # substring check, which — unlike the exact-match `replied?`/`sent?` — also
  # catches provider-synced sent mail that carries a display name).
  #
  # follow_up_* hold the AI verdict (one active follow-up per thread; recomputed as
  # mail arrives), kept on the thread rather than a side table so the surfacing
  # query is a single indexed read.
  def up
    change_table :email_threads, bulk: true do |t|
      t.datetime :last_outbound_at
      t.datetime :last_inbound_at
      t.boolean  :follow_up_expected, null: false, default: false
      t.datetime :follow_up_at
      t.string   :follow_up_reason
      t.datetime :follow_up_last_analyzed_at
      t.datetime :follow_up_dismissed_at
      t.bigint   :follow_up_outbound_message_id
    end

    # The hot read path: pending, not-dismissed follow-ups that have come due.
    add_index :email_threads, :follow_up_at,
              where: "follow_up_expected AND follow_up_dismissed_at IS NULL",
              name: "index_email_threads_on_due_follow_ups"

    # Backfill the denormalized reply timestamps from existing mail in one grouped
    # UPDATE. A message is the owner's when its from_address contains the account
    # address (strpos > 0) — same semantics the runtime maintenance uses. Skipped in
    # test (schema.rb load never runs migrations); factories set the columns directly.
    execute(<<~SQL.squish)
      UPDATE email_threads et SET
        last_outbound_at = agg.last_out,
        last_inbound_at  = agg.last_in
      FROM (
        SELECT em.email_thread_id AS thread_id,
               MAX(CASE WHEN strpos(LOWER(em.from_address), LOWER(ea.email_address)) > 0
                        THEN em.received_at END) AS last_out,
               MAX(CASE WHEN strpos(LOWER(em.from_address), LOWER(ea.email_address)) = 0
                        THEN em.received_at END) AS last_in
        FROM email_messages em
        JOIN email_accounts ea ON ea.id = em.email_account_id
        WHERE em.email_thread_id IS NOT NULL
        GROUP BY em.email_thread_id
      ) agg
      WHERE et.id = agg.thread_id
    SQL
  end

  def down
    remove_index :email_threads, name: "index_email_threads_on_due_follow_ups"
    change_table :email_threads, bulk: true do |t|
      t.remove :last_outbound_at, :last_inbound_at, :follow_up_expected, :follow_up_at,
               :follow_up_reason, :follow_up_last_analyzed_at, :follow_up_dismissed_at,
               :follow_up_outbound_message_id
    end
  end
end
