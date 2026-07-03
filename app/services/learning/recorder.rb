module Learning
  # Writes a LearningDecision for an ephemeral suggestion the user acted on or
  # dismissed — a domain (Skim cluster, feed tag-suggestion) that has no durable
  # verdict record of its own. The CALLER derives the signals server-side from a
  # scoped record (never from client params); this just persists them.
  #
  # Best-effort and non-fatal: a failure to record must never break the user's
  # action, so every error is swallowed and logged (mirrors SkimDecisionRecorder).
  module Recorder
    module_function

    def record(domain:, user:, workspace_id:, label:, subject: nil,
               contact_id: nil, sender_domain: nil, category: nil, signals: {})
      LearningDecision.create!(
        domain: domain, user: user, workspace_id: workspace_id, label: label.to_s,
        subject: subject, contact_id: contact_id, sender_domain: sender_domain,
        category: category, signals: signals || {}
      )
    rescue => e
      Rails.logger.warn("[Learning::Recorder] #{e.class}: #{e.message}")
      nil
    end
  end
end
