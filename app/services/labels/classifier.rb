module Labels
  # Deterministic, no-AI classification of a synced provider label. Returns a
  # decision hash for labels we recognise as provider system statuses / category
  # tabs, or nil for the ambiguous long tail (which Labels::AiClassifier judges).
  #
  # The Gmail id list mirrors Google::MailClient#system_label?. Zoho exposes no
  # system flag on its /labels endpoint, so we match its built-in folder names.
  class Classifier
    GMAIL_SYSTEM_IDS = %w[
      INBOX SENT DRAFT TRASH SPAM IMPORTANT STARRED UNREAD
      CATEGORY_PERSONAL CATEGORY_SOCIAL CATEGORY_PROMOTIONS
      CATEGORY_UPDATES CATEGORY_FORUMS CHAT SNOOZED
    ].freeze

    GMAIL_CATEGORY_IDS = %w[
      CATEGORY_PERSONAL CATEGORY_SOCIAL CATEGORY_PROMOTIONS
      CATEGORY_UPDATES CATEGORY_FORUMS
    ].freeze

    ZOHO_SYSTEM_NAMES = %w[
      Inbox Sent Drafts Trash Spam Archive Outbox Templates Snoozed
    ].map(&:downcase).freeze

    def initialize(tag)
      @tag = tag
    end

    # Returns { kind:, hidden:, confidence:, reason: } or nil (nil ⇒ defer to AI).
    def classify
      if gmail_category?
        decision(:category, "Gmail category tab")
      elsif gmail_system? || zoho_system? || @tag.system_label?
        decision(:system, provider_system_reason)
      end
    end

    private

    def decision(kind, reason)
      { kind: kind, hidden: true, confidence: 1.0, reason: reason }
    end

    def gmail?
      @tag.email_account&.google?
    end

    def zoho?
      @tag.email_account&.zoho?
    end

    def label_id
      @tag.external_label_id.to_s
    end

    def gmail_system?
      gmail? && GMAIL_SYSTEM_IDS.include?(label_id)
    end

    def gmail_category?
      gmail? && GMAIL_CATEGORY_IDS.include?(label_id)
    end

    def zoho_system?
      zoho? && ZOHO_SYSTEM_NAMES.include?(@tag.name.to_s.strip.downcase)
    end

    def provider_system_reason
      zoho? ? "Zoho system folder" : "Gmail system label"
    end
  end
end
