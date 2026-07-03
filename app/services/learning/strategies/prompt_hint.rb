module Learning
  module Strategies
    # One "way to train": turn a consensus Suggestion into a natural-language
    # line injected into an LLM system prompt, so the model is biased by how the
    # human has decided before. All the hint copy lives here (one place) to stay
    # consistent; each domain has its own phrasing because what a document
    # classification hint says differs from what a reminder suppression hint says.
    #
    # Generalizes the inline hint that used to live in
    # Documents::ClassificationMemory#prompt_hint.
    module PromptHint
      module_function

      # Document classification: nudge toward the type the human keeps approving.
      def for_documents(suggestion, type_name:)
        origin = suggestion.source == :sender ? "from this sender" : "with similar filenames"
        "Learned from past human-approved documents in this workspace: " \
          "#{suggestion.count} of #{suggestion.total} documents #{origin} were classified as " \
          "\"#{type_name}\". Strongly prefer \"#{type_name}\" unless the content clearly " \
          "indicates a different type."
      end

      # Reminder extraction: warn the model off a type of dated commitment the
      # reader keeps dismissing from senders like this one.
      def for_reminders(suggestion, reminder_type:)
        label = reminder_type.to_s.humanize(capitalize: false)
        "The workspace has dismissed #{suggestion.count} of #{suggestion.total} #{label} " \
          "reminders from senders like this — only extract one if the obligation is concrete, " \
          "explicit, and the date is unambiguous."
      end

      # Task extraction: warn the model off suggesting tasks from a sender whose
      # AI-suggested tasks the reader keeps cancelling.
      def for_tasks(suggestion)
        "The workspace has dismissed #{suggestion.count} of #{suggestion.total} AI-suggested " \
          "tasks from senders like this — only extract a task if the action is explicit, direct, " \
          "and clearly owned by the reader."
      end
    end
  end
end
