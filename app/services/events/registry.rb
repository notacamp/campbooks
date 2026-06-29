module Events
  # Single source of truth for the KNOWN domain event types — the catalog the
  # workflow trigger picker, the Liquid variable hints, and the activity feed
  # read from. Mirrors Workflows::ActionRegistry.
  #
  # Emission is NOT restricted to these keys: Events.publish accepts any string
  # (custom workflow emit_event, future external sources). Registered keys get a
  # friendly label/icon/group in the UI; unregistered ones fall back to a
  # humanized name. Adding a tracked event = add one Definition here + the
  # Events.publish call at its source.
  #
  # `label`/`description` are the English source; the activity feed resolves a
  # user-facing label via i18n (events.names.<underscored_key>) and uses these as
  # the default. `payload_keys` documents which Liquid variables a step can read
  # off the event payload.
  module Registry
    Definition = Struct.new(
      :key, :label, :group, :icon, :description, :subject_type, :payload_keys,
      keyword_init: true
    ) do
      # i18n-safe key fragment ("email.received" -> "email_received").
      def i18n_key
        key.tr(".", "_")
      end

      # Shape consumed by the trigger picker.
      def picker_card
        { group: group, key: key, title: label, icon: icon, description: description }
      end

      # [value, label] pair for the trigger event <select>.
      def select_option
        [ key, label ]
      end
    end

    def self.entry(key, label, group:, icon:, description:, subject_type: nil, payload_keys: [])
      Definition.new(
        key: key, label: label, group: group, icon: icon, description: description,
        subject_type: subject_type, payload_keys: payload_keys
      )
    end

    DEFINITIONS = [
      # --- Email ---------------------------------------------------------------
      entry("email.received", "Email received", group: :email, icon: :mail,
        description: "A new email finished processing", subject_type: "EmailMessage",
        payload_keys: %w[subject from to account_email]),
      entry("email.archived", "Email archived", group: :email, icon: :archive,
        description: "An email was archived", subject_type: "EmailMessage", payload_keys: %w[subject from]),
      entry("email.trashed", "Email trashed", group: :email, icon: :trash,
        description: "An email was moved to trash", subject_type: "EmailMessage", payload_keys: %w[subject from]),
      entry("email.snoozed", "Email snoozed", group: :email, icon: :clock,
        description: "An email was snoozed", subject_type: "EmailMessage", payload_keys: %w[subject until]),
      entry("email.tagged", "Email tagged", group: :email, icon: :tag,
        description: "A tag was added to an email", subject_type: "EmailMessage", payload_keys: %w[subject tag]),
      entry("email.forwarded", "Email forwarded", group: :email, icon: :send,
        description: "An email was forwarded", subject_type: "EmailMessage", payload_keys: %w[subject to]),
      entry("email.sent", "Email sent", group: :email, icon: :send,
        description: "An email was sent from a connected account", subject_type: "EmailMessage",
        payload_keys: %w[subject to]),
      entry("email.bulk_archived", "Emails bulk-archived", group: :email, icon: :archive,
        description: "Several emails were archived at once", payload_keys: %w[count ids]),
      entry("email.skim_decision", "Skim decision", group: :email, icon: :inbox,
        description: "A keep/archive/promote decision was made in Skim", subject_type: "EmailMessage",
        payload_keys: %w[decision subject]),

      # --- Documents -----------------------------------------------------------
      entry("document.processed", "Document processed", group: :documents, icon: :document,
        description: "AI finished analyzing a document", subject_type: "Document",
        payload_keys: %w[filename document_type]),
      entry("document.approved", "Document approved", group: :documents, icon: :check,
        description: "A document was approved in review", subject_type: "Document",
        payload_keys: %w[filename document_type]),
      entry("document.rejected", "Document rejected", group: :documents, icon: :x,
        description: "A document was rejected in review", subject_type: "Document", payload_keys: %w[filename]),
      entry("document.restored", "Document restored", group: :documents, icon: :document,
        description: "A document was restored to the review queue", subject_type: "Document",
        payload_keys: %w[filename]),

      # --- Pipelines ----------------------------------------------------------
      entry("pipeline.stage_entered", "Stage entered", group: :pipelines, icon: :git_branch,
        description: "A document or email entered a pipeline stage",
        payload_keys: %w[pipeline_name stage_name stage_id pipeline_id]),

      # --- Calendar ------------------------------------------------------------
      entry("calendar_event.created", "Calendar event created", group: :calendar, icon: :calendar,
        description: "A calendar event was created", subject_type: "CalendarEvent",
        payload_keys: %w[title starts_at]),
      entry("calendar_event.updated", "Calendar event updated", group: :calendar, icon: :calendar,
        description: "A calendar event was updated", subject_type: "CalendarEvent",
        payload_keys: %w[title starts_at]),
      entry("calendar_event.deleted", "Calendar event deleted", group: :calendar, icon: :calendar,
        description: "A calendar event was deleted", payload_keys: %w[title]),

      # --- Contacts ------------------------------------------------------------
      entry("contact.starred", "Contact starred", group: :contacts, icon: :star,
        description: "A contact was starred", subject_type: "Contact", payload_keys: %w[name email]),
      entry("contact.unstarred", "Contact unstarred", group: :contacts, icon: :star,
        description: "A contact was unstarred", subject_type: "Contact", payload_keys: %w[name email]),
      entry("contact.blocked", "Contact blocked", group: :contacts, icon: :ban,
        description: "A contact was blocked", subject_type: "Contact", payload_keys: %w[name email]),
      entry("contact.unblocked", "Contact unblocked", group: :contacts, icon: :ban,
        description: "A contact was unblocked", subject_type: "Contact", payload_keys: %w[name email]),

      # --- Reminders -----------------------------------------------------------
      entry("reminder.created", "Reminder created", group: :reminders, icon: :bell,
        description: "A reminder was extracted from email or a document", subject_type: "Reminder",
        payload_keys: %w[title due_at]),
      entry("reminder.confirmed", "Reminder confirmed", group: :reminders, icon: :bell,
        description: "A reminder was confirmed into a calendar event", subject_type: "Reminder",
        payload_keys: %w[title due_at]),
      entry("reminder.dismissed", "Reminder dismissed", group: :reminders, icon: :bell,
        description: "A reminder was dismissed", subject_type: "Reminder", payload_keys: %w[title]),

      # --- Tasks ---------------------------------------------------------------
      entry("task.created", "Task created", group: :tasks, icon: :check_square,
        description: "A task was created (manually or extracted from email)", subject_type: "Task",
        payload_keys: %w[title status ai_suggested]),
      entry("task.status_changed", "Task status changed", group: :tasks, icon: :clock,
        description: "A task moved to a new status", subject_type: "Task",
        payload_keys: %w[title from to]),
      entry("task.assigned", "Task assigned", group: :tasks, icon: :user,
        description: "A task was assigned to a member", subject_type: "Task",
        payload_keys: %w[title assignee_id]),
      entry("task.completed", "Task completed", group: :tasks, icon: :check,
        description: "A task was completed", subject_type: "Task",
        payload_keys: %w[title]),
      entry("task.archived", "Task archived", group: :tasks, icon: :archive,
        description: "A task was archived", subject_type: "Task",
        payload_keys: %w[title]),
      entry("task.unarchived", "Task restored", group: :tasks, icon: :inbox,
        description: "An archived task was restored", subject_type: "Task",
        payload_keys: %w[title]),

      # --- Account / integrations ---------------------------------------------
      entry("email_account.connected", "Account connected", group: :account, icon: :plug,
        description: "An email account was connected", subject_type: "EmailAccount",
        payload_keys: %w[email_address provider]),
      entry("email_account.disconnected", "Account disconnected", group: :account, icon: :plug,
        description: "An email account was disconnected", subject_type: "EmailAccount",
        payload_keys: %w[email_address provider])
    ].freeze

    INDEX = DEFINITIONS.index_by(&:key).freeze
    GROUPS = DEFINITIONS.map(&:group).uniq.freeze

    class << self
      def all
        DEFINITIONS
      end

      def definition(key)
        INDEX[key.to_s]
      end

      def keys
        DEFINITIONS.map(&:key)
      end

      def groups
        GROUPS
      end

      # [[key, label], ...] for the trigger event <select>.
      def select_options
        DEFINITIONS.map(&:select_option)
      end

      # { group => [[key, label], ...] } for an optgroup-style picker.
      def grouped_select_options
        DEFINITIONS.group_by(&:group).transform_values { |defs| defs.map(&:select_option) }
      end

      def picker_cards
        DEFINITIONS.map(&:picker_card)
      end

      def payload_keys_for(key)
        definition(key)&.payload_keys || []
      end
    end
  end
end
