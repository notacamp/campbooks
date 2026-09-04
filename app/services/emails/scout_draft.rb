# frozen_string_literal: true

module Emails
  # Shared lookup for the freshest non-outdated Scout reply draft on an email
  # thread. Extracted so PeopleController and EmailComposeController resolve the
  # same draft and can never disagree.
  #
  # A draft is a non-outdated AI AgentMessage with an empty suggested-actions
  # array (question prompts also appear as drafts but carry ai_suggested_actions
  # — they are excluded so they never ghost into a compose canvas or the People
  # conversation pane).
  module ScoutDraft
    # Returns the content string of the freshest draft, or nil.
    def self.for(message)
      return nil unless message&.email_thread

      agent_thread = message.email_thread.agent_thread
      return nil unless agent_thread

      agent_thread.agent_messages
                  .where(draft: true, outdated: false, author_type: :ai)
                  .where("ai_suggested_actions = '[]'::jsonb")
                  .order(created_at: :desc)
                  .first
                  &.content
    end
  end
end
