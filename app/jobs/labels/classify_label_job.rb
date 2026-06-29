module Labels
  # Classifies one synced provider label's value and remembers the decision
  # (classified_at), so the inbox only shows labels worth showing. Enqueued by the
  # label sync services for each newly-seen label, and by the backfill rake for
  # the existing long tail.
  #
  # Idempotent: a re-enqueue for an already-classified tag is a no-op, so it's
  # safe to call from concurrent syncs and to re-run the backfill.
  class ClassifyLabelJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    # Sync-time entry point. Decide a label's visibility now if it's a known
    # provider system/category label; otherwise enqueue the async AI judgment.
    # Idempotent — a tag that's already been classified is left untouched.
    def self.classify(tag)
      return if tag.classified_at.present?

      if (decision = Labels::Classifier.new(tag).classify)
        tag.apply_classification!(**decision)
      else
        perform_later(tag.id)
      end
    end

    def perform(tag_id)
      tag = Tag.find(tag_id)
      return if tag.classified_at.present?

      workspace = tag.workspace
      Current.workspace = workspace

      # Deterministic rules first (covers the case the sync didn't pre-classify).
      if (decision = Labels::Classifier.new(tag).classify)
        apply(tag, decision)
        return
      end

      # No usable text provider → keep visible. We never hide a genuine user label
      # without an actual judgment; rules still hid the obvious system ones above.
      # Stamp classified_at so a keyless workspace isn't re-checked every sync.
      unless Ai::ProviderSetup.configured?(workspace, :text)
        apply(tag, keep_visible)
        return
      end

      # AI judges the long tail; on any failure/unavailability, fail open (visible).
      apply(tag, Labels::AiClassifier.new(tag).classify || keep_visible)
    rescue ActiveRecord::RecordNotFound
      Rails.logger.warn("[Labels::ClassifyLabelJob] Tag #{tag_id} not found, skipping")
    ensure
      Current.workspace = nil
    end

    private

    def keep_visible
      { kind: :user, hidden: false, confidence: nil, reason: nil }
    end

    def apply(tag, decision)
      became_hidden = decision[:hidden] && !tag.hidden?
      tag.apply_classification!(**decision)

      # Just became hidden → drop its (now-noise) per-message assignments.
      EmailMessageTag.where(tag_id: tag.id).delete_all if became_hidden
    end
  end
end
