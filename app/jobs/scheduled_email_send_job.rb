# frozen_string_literal: true

class ScheduledEmailSendJob < ApplicationJob
  queue_as :default

  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform
    ScheduledEmail.due.find_each do |scheduled_email|
      process(scheduled_email)
    rescue => e
      Rails.logger.error("[ScheduledEmailSendJob] Failed for ##{scheduled_email.id}: #{e.message}")
      scheduled_email.update_columns(status: ScheduledEmail.statuses[:failed]) if scheduled_email.pending?
    end
  end

  private

  # Templated scheduled emails regenerate their document-template PDFs on every
  # occurrence (so a recurring send always carries fresh attachments). Plain
  # scheduled emails carry none.
  def attachments_for(scheduled_email)
    return [] unless scheduled_email.email_template

    EmailTemplates::Applier.pdf_attachments(
      template: scheduled_email.email_template,
      variables: scheduled_email.template_context
    )
  end

  def process(scheduled_email)
    result = Emails::Sender.call(
      user: scheduled_email.created_by,
      email_account_id: scheduled_email.email_account_id,
      to_address: scheduled_email.to_address,
      subject: scheduled_email.rendered_subject,
      body: scheduled_email.rendered_body,
      cc_address: scheduled_email.cc_address.presence,
      bcc_address: scheduled_email.bcc_address.presence,
      attachments: attachments_for(scheduled_email)
    )

    if result.ok?
      handle_success(scheduled_email)
    else
      scheduled_email.update_columns(status: ScheduledEmail.statuses[:failed])
    end
  end

  def handle_success(scheduled_email)
    now = Time.current

    if scheduled_email.rrule.present?
      next_time = ScheduleCalculator.next_occurrence(scheduled_email.scheduled_at, scheduled_email.rrule, now)
      if next_time && next_time > now
        scheduled_email.update_columns(
          status: ScheduledEmail.statuses[:pending],
          last_sent_at: now,
          scheduled_at: next_time,
          next_occurrence_at: next_time
        )
      else
        scheduled_email.update_columns(status: ScheduledEmail.statuses[:sent], last_sent_at: now)
      end
    else
      scheduled_email.update_columns(status: ScheduledEmail.statuses[:sent], last_sent_at: now)
    end
  end
end
