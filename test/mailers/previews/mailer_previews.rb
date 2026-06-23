class MailerPreviews < ActionMailer::Preview
  def invitation
    org = Workspace.new(id: 1, name: "Not A Camp")
    invited_by = User.new(id: 1, name: "Alex", email_address: "alex@example.com")
    invitation = Invitation.new(
      id: 1,
      email: "colleague@example.com",
      token: "fake-token-123",
      workspace: org,
      invited_by: invited_by
    )
    InvitationMailer.invitation(invitation)
  end

  def password_reset
    user = User.new(
      id: 1,
      name: "Alex",
      email_address: "alex@example.com"
    )
    def user.password_reset_token
      "fake-reset-token-123"
    end
    def user.password_reset_token_expires_in
      15.minutes
    end
    PasswordsMailer.reset(user)
  end

  def verification
    VerificationMailer.verify(
      email_address: "alex@example.com",
      code: "482917",
      name: "Alex"
    )
  end
end
