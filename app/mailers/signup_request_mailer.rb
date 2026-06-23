class SignupRequestMailer < ApplicationMailer
  def approved(signup_request)
    @signup_request = signup_request
    @url = registration_approved_url(token: signup_request.token)

    with_recipient_locale(nil) do
      mail(to: signup_request.email, subject: t(".subject"))
    end
  end
end
