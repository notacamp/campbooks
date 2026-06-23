class InvitationMailer < ApplicationMailer
  def invitation(invitation)
    @invitation = invitation
    @workspace = invitation.workspace
    @invited_by = invitation.invited_by

    with_recipient_locale(@invited_by) do
      mail(
        to: invitation.email,
        subject: t(".subject", inviter: @invited_by.name, workspace: @workspace.name)
      )
    end
  end
end
