class PasswordsMailer < ApplicationMailer
  def reset(user)
    @user = user
    with_recipient_locale(user) do
      mail subject: t(".subject"), to: user.email_address
    end
  end
end
