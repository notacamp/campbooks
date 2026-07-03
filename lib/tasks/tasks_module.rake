# Maintenance for the Tasks module's AI extraction. (Pruning machine-mail
# suggestions needs no rake — Tasks::PruneAutomatedSuggestionsJob self-heals via
# migration; backfill stays manual because it spends LLM calls.)
namespace :tasks do
  desc "Re-enqueue task extraction for recent mail the gate admits (DAYS=7 back, " \
       "DRY_RUN=true by default). Backfills mail ingested before the Tasks module was " \
       "enabled — or whose extraction was lost to a transient provider error. The job " \
       "re-checks the feature flag, entitlement and gate; the builder is idempotent."
  task backfill_extraction: :environment do
    days    = ENV.fetch("DAYS", "7").to_i
    dry_run = ENV.fetch("DRY_RUN", "true") != "false"
    banner  = dry_run ? "DRY RUN (no writes — set DRY_RUN=false to apply)" : "ENQUEUING"
    puts "=== tasks:backfill_extraction — #{banner} (DAYS=#{days}) ==="

    enqueued = skipped = 0
    EmailMessage.where(created_at: days.days.ago..)
                .where.not(category: Tasks::ExtractionGate::MACHINE_CATEGORIES)
                .find_each do |email|
      next skipped += 1 unless Tasks::ExtractionGate.email_allows?(email)
      # Already produced tasks for this email — nothing to backfill.
      next skipped += 1 if Task.exists?(source_type: "EmailMessage", source_id: email.id)

      enqueued += 1
      puts "  ENQUEUE #{email.id} #{email.subject.to_s.truncate(60).inspect}"
      Tasks::EmailExtractionJob.perform_later(email.id) unless dry_run
    end

    puts "=== enqueued #{enqueued}, skipped #{skipped}#{" (dry run)" if dry_run} ==="
  end
end
