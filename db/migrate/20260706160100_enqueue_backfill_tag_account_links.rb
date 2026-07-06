# frozen_string_literal: true

# Backfills tag_account_links and label_import_decisions for existing external
# tags (those with email_account_id + external_label_id set). Runs off the boot
# critical path via a job so a large installation never stalls db:prepare.
# Best-effort and non-fatal.
class EnqueueBackfillTagAccountLinks < ActiveRecord::Migration[8.1]
  def up
    Tags::BackfillAccountLinksJob.perform_later
  rescue StandardError => e
    say "Skipped tag-account-links backfill enqueue (#{e.class}: #{e.message}); run Tags::BackfillAccountLinksJob manually."
  end

  def down
    # No-op: links and decisions are reconstructable by re-running the job.
  end
end
