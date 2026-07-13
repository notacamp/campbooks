class ApplicationMailer < ActionMailer::Base
  # Set MAILER_FROM to a sender your SMTP provider authorizes (with some providers,
  # e.g. Zoho, it must be the authenticated mailbox or a verified send-as alias).
  #
  # X-Campbooks-Kind marks every outgoing app email as ours. This mail is delivered
  # to the user's own mailbox and re-ingested by the scanner; the marker lets
  # Emails::SelfGeneratedDetector recognise it (and skip the AI pipeline) even when
  # MAILER_FROM is a shared no-reply@ address that other services also send from.
  # DigestMailer overrides it to "digest" so the inbox can badge digests.
  default from: ENV.fetch("MAILER_FROM", "Campbooks <no-reply@example.com>"),
          "X-Campbooks-Kind" => "campbooks"
  layout "mailer"
  helper :mailer_style

  private

  # Render the email in the recipient's preferred language. Accepts a record that
  # responds to #locale (a User) or a literal locale; nil falls back to the app
  # default (pre-account emails like verification have no stored preference).
  # ActiveJob does not preserve I18n.locale across deliver_later, so each mailer
  # sets it explicitly here, at render time.
  def with_recipient_locale(source, &block)
    locale = source.respond_to?(:locale) ? source.locale : source
    I18n.with_locale(locale.presence || I18n.default_locale, &block)
  end
end
