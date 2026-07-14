# frozen_string_literal: true

# The Files grid view renders a thumbnail per document; new documents generate
# theirs on create, and this one-shot sweep covers everything that already
# exists — so the grid works on upgrade without anyone running a rake task
# (self-hosted upgrades must be one step). The rendering runs on the worker as
# a single idempotent job, kept OFF the boot / db:prepare critical path.
#
# Best-effort and non-fatal: new documents thumbnail themselves regardless, so
# a failed enqueue (e.g. the queue isn't reachable during boot) must not break
# the migration/deploy.
class EnqueueDocumentThumbnailBackfill < ActiveRecord::Migration[8.1]
  def up
    Documents::ThumbnailBackfillJob.perform_later
  rescue StandardError => e
    say "Skipped document-thumbnail backfill enqueue (#{e.class}: #{e.message}); run Documents::ThumbnailBackfillJob manually."
  end

  def down
    # No-op: thumbnails are reconstructable by re-running the backfill.
  end
end
