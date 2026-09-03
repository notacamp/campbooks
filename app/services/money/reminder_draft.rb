# frozen_string_literal: true

class Money
  # The plain-text email Scout drafts when you chase a late receivable ("Send
  # reminder") or ask to cancel a renewal ("Cancel it"). Templated, not AI — polite,
  # four sentences, with the amount, due date and how many days late. Pure: given an
  # obligation it returns a subject + body; the recipient is resolved by the
  # controller (a contact lookup), and the whole thing lands editable in the compose
  # Dock, so the wording is always the user's to change before anything sends.
  #
  #   draft = Money::ReminderDraft.chase(obligation)
  #   draft.subject   # "Invoice #0231 · payment reminder"
  #   draft.body      # four-sentence plain-text reminder
  class ReminderDraft
    Draft = Struct.new(:subject, :body, keyword_init: true)

    def self.chase(obligation, today: Date.current)
      new(obligation, today).chase
    end

    def self.cancellation(obligation, today: Date.current)
      new(obligation, today).cancellation
    end

    def initialize(obligation, today = Date.current)
      @obligation = obligation
      @today = today
    end

    def chase
      Draft.new(subject: chase_subject, body: chase_body)
    end

    def cancellation
      Draft.new(
        subject: I18n.t("money.reminder_draft.cancellation.subject", counterpart: counterpart),
        body:    I18n.t("money.reminder_draft.cancellation.body", counterpart: counterpart, date: formatted_due)
      )
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
        amount:      formatted_amount,
        due:         formatted_due,
        days_ago:    I18n.t("money.reminder_draft.days_ago", count: @obligation.days_late(@today))
      )
    end

    def counterpart = @obligation.counterpart.to_s

    def reference = @obligation.document&.invoice_number.to_s

    def formatted_amount = @obligation.amount&.format || "-"

    def formatted_due
      return "" if @obligation.due_on.blank?

      I18n.l(@obligation.due_on, format: :day_month)
    end
  end
end
