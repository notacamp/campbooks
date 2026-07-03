class EnqueueAutomatedTaskSuggestionPrune < ActiveRecord::Migration[8.1]
  # The task-extraction gate now vetoes machine mail (no-reply senders,
  # notification/promo/social categories, outbound mail), but suggestions minted
  # before that fix are still sitting in triage — GitHub bots, marketplace CTAs,
  # security alerts. Rather than asking operators to run a rake task after
  # upgrading — this is self-hosted, upgrades must be one step — enqueue the
  # idempotent cleanup job so it runs on the worker, off the boot/db:prepare
  # critical path.
  #
  # Best-effort and non-fatal: a failed enqueue (e.g. the queue isn't reachable
  # yet during boot) must not break the migration/deploy; the job can be enqueued
  # again at any time.
  def up
    Tasks::PruneAutomatedSuggestionsJob.perform_later
  rescue StandardError => e
    say "Skipped task-suggestion prune enqueue (#{e.class}: #{e.message}); run Tasks::PruneAutomatedSuggestionsJob manually."
  end

  def down
    # No-op: dismissed suggestions stay dismissed (they were machine-mail noise).
  end
end
