class VerificationMailer < ApplicationMailer
  def verify(email_address:, code:, name:)
    @code = code
    @name = name.presence || email_address.split("@").first
    with_recipient_locale(nil) do
      mail subject: t(".subject", code: @code), to: email_address
    end
  end
end
