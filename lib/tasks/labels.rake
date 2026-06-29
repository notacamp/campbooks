namespace :labels do
  desc "Classify the value of already-synced provider labels (Gmail/Zoho) once and remember the decision. " \
       "Known system/category labels resolve inline; the ambiguous long tail enqueues Labels::ClassifyLabelJob (AI). " \
       "Idempotent — skips already-classified labels. ENV: LIMIT=<n>, WORKSPACE_ID=<id>."
  task classify_existing: :environment do
    limit        = ENV["LIMIT"]&.to_i
    workspace_id = ENV["WORKSPACE_ID"]&.to_i

    scope = Tag.external.where(classified_at: nil)
    scope = scope.where(workspace_id: workspace_id) if workspace_id&.positive?
    scope = scope.limit(limit) if limit&.positive?

    total = resolved = enqueued = 0
    scope.find_each do |tag|
      total += 1
      Labels::ClassifyLabelJob.classify(tag)
      # Deterministic decisions set classified_at in place; ambiguous ones are
      # left for the async AI job (classified_at still nil here).
      tag.classified_at.present? ? resolved += 1 : enqueued += 1
    end

    puts "Processed #{total} unclassified provider label(s): " \
         "#{resolved} resolved inline (system/category), #{enqueued} enqueued for AI classification."
    puts "After the queue drains, run `rails labels:cleanup_hidden_assignments` to drop hidden-label message rows."
  end

  desc "Drop EmailMessageTag rows for hidden labels (provider system / AI low-value). " \
       "Join-rows only — reconstructable by re-sync. Run AFTER labels:classify_existing has drained."
  task cleanup_hidden_assignments: :environment do
    deleted = Labels::CleanupHiddenAssignmentsJob.new.perform
    puts "Removed #{deleted} hidden-label message assignment(s)."
  end
end
