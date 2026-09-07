# frozen_string_literal: true

module Emails
  # Decides whether to skip all AI for an email based on which mail-provider
  # folder it lives in. Spam, Junk, and Trash are unwanted-mail folders the
  # provider already sorted for us — running triage/embedding/reminders on them
  # wastes LLM calls and manufactures ghost tasks/contacts from noise.
  #
  # The check is folder-name-based (stored in email_folders.name) rather than
  # provider-ID-based, so it works across Zoho, Gmail (IMAP), and any future
  # provider whose folder names we normalise at sync time.
  #
  # Conservative: returns false (allow AI) when the folder lookup fails or when
  # the email has no provider_folder_id, so a transient miss or an unmapped
  # folder never silently drops a real person's mail from the AI pipeline.
  class FolderGate
    # Canonical folder names (matched case-insensitively) that indicate the
    # provider already decided the mail is unwanted.
    # Kept in sync with EmailFolder::DEFAULT_ORDER.
    SKIP_FOLDER_NAMES = %w[spam junk trash].freeze

    def self.skip_ai?(email)
      new(email).skip_ai?
    end

    def initialize(email)
      @email = email
    end

    def skip_ai?
      return false if @email.provider_folder_id.blank?

      SKIP_FOLDER_NAMES.include?(folder_name&.downcase)
    end

    private

    # Look up the EmailFolder row for this email's account + provider_folder_id.
    # Returns nil when no matching row exists (e.g. folder not yet mirrored).
    def folder_name
      @email.email_account
            .email_folders
            .find_by(provider_folder_id: @email.provider_folder_id)
            &.name
    end
  end
end
