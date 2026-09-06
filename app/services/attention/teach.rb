# frozen_string_literal: true

module Attention
  # Records "this person matters / doesn't matter" as a LearningDecision in the
  # `attention` domain (one row per contact per call, newest wins in Attention::Signals),
  # then refreshes the user's weights so the change shows within the request's
  # next paint. Verdicts are per user, like every other attention signal.
  module Teach
    DOMAIN = "attention"
    LABELS = %w[important unimportant].freeze

    # person: a Person (all of its contacts get the verdict so aliases agree)
    # label:  "important" | "unimportant"
    # source: "rail" | "memory" | "teach"
    # → true/false
    def self.record(person:, user:, label:, source: "rail")
      workspace_id = user.workspace_id
      person.contacts.each do |contact|
        Learning::Recorder.record(
          domain:       DOMAIN,
          user:         user,
          workspace_id: workspace_id,
          label:        label.to_s,
          contact_id:   contact.id,
          subject:      person,
          sender_domain: Emails::SenderDomain.for(contact.email),
          signals:      { "source" => source.to_s }
        )
      end
      refresh!(user)
      true
    end

    # Forget every taught verdict for the person (weight falls back to behaviour).
    def self.forget(person:, user:)
      LearningDecision
        .where(domain: DOMAIN, user_id: user.id, contact_id: person.contacts.ids)
        .delete_all
      refresh!(user)
      true
    end

    # The current verdict for a person (newest row across its contacts), or nil.
    def self.verdict(person:, user:)
      return nil unless user
      LearningDecision
        .where(domain: DOMAIN, user_id: user.id, contact_id: person.contacts.ids)
        .order(created_at: :desc)
        .first&.label
    end

    def self.refresh!(user)
      Attention::Refresh.call(user)
      People::StandingsRefreshJob.enqueue_for(user.id)
      Feed::RefreshJob.enqueue_for(user.id)
    end
    private_class_method :refresh!
  end
end
