# frozen_string_literal: true

class Money
  # The plain-text email Scout drafts when you chase a receivable ("Send reminder").
  # Templated, not AI: polite, a few sentences, with the amount and invoice reference.
  # Pure: given an obligation it returns a subject + body; the recipient is resolved by
  # the controller (a contact lookup), and the result lands editable in the compose
  # Dock, so the wording is always the user's to change before anything sends.
  #
  #   draft = Money::ReminderDraft.chase(obligation)
  #   draft.subject   # "Invoice #0231 payment reminder"
  #   draft.body      # plain-text reminder asking when to expect the payment
  class ReminderDraft
    Draft = Struct.new(:subject, :body, keyword_init: true)

    def self.chase(obligation, today: Date.current) # rubocop:disable Lint/UnusedMethodArgument
      new(obligation).chase
    end

    def initialize(obligation)
      @obligation = obligation
    end

    def chase
      Draft.new(subject: chase_subject, body: chase_body)
    end

    private

    def chase_subject
      if reference.present?
        I18n.t("money.reminder_draft.chase.subject", reference: reference)
      else
        I18n.t("money.reminder_draft.chase.subject_generic")
      end
    end

    def chase_body
      I18n.t(
        "money.reminder_draft.chase.body",
        counterpart: counterpart,
        reference:   reference.present? ? I18n.t("money.reminder_draft.chase.invoice_ref", reference: reference) : I18n.t("money.reminder_draft.chase.this_invoice"),
        amount:      formatted_amount
      )
    end

    def counterpart    = @obligation.counterpart.to_s
    def reference      = @obligation.document&.invoice_number.to_s
    def formatted_amount = @obligation.amount&.format || "-"
  end
end
