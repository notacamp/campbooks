# frozen_string_literal: true

# Ship the four default tag groups to existing workspaces and tag their already-
# categorized mail, so the inbox collapse works on upgrade without anyone running
# a rake task (self-hosted upgrades must be one step). The potentially-large
# insert runs on the worker via an idempotent, batched job — kept OFF the boot /
# db:prepare critical path so a big mailbox never stalls the deploy.
#
# Best-effort and non-fatal: the category->tag bridge self-heals go-forward mail
# regardless, so a failed enqueue (e.g. the queue isn't reachable during boot)
# must not break the migration/deploy.
class EnqueueDefaultBucketTagsBackfill < ActiveRecord::Migration[8.1]
  def up
    Tags::BackfillDefaultBucketTagsJob.perform_later
  rescue StandardError => e
    say "Skipped default-bucket-tags backfill enqueue (#{e.class}: #{e.message}); run Tags::BackfillDefaultBucketTagsJob manually."
  end

  def down
    # No-op: the attached tags are reconstructable by re-running the backfill.
  end
end
