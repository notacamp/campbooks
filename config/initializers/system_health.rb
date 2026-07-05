# frozen_string_literal: true

# Records transactional email deliveries to the SystemHealth log so the admin
# dashboard can track SMTP success/error rates.
#
# Skips recording in :test delivery mode — mail is never actually sent then,
# and logging synthetic no-ops would dilute real signal.
#
# Recipient addresses are never recorded (they never enter the event payload).
ActiveSupport::Notifications.subscribe("deliver.action_mailer") do |event|
  next if ActionMailer::Base.delivery_method == :test

  ex = event.payload[:exception_object]
  SystemHealth.record(
    service:       "smtp",
    operation:     event.payload[:mailer] || "deliver",
    status:        ex ? :error : :success,
    duration_ms:   event.duration.round,
    error_class:   ex&.class&.name,
    error_message: ex && SystemHealth.sanitize_message(ex.message)
  )
end
