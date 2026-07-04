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

  # Renders the daily "needs attention" digest. Uses live data from the seed user
  # when available; falls back to sample records so all three sections are always
  # visible in the preview (no real emails sent to or from real people).
  def needs_attention_digest
    user = User.find_by(email_address: "admin@example.com") || User.first
    if user
      thread_ids   = Emails::AwaitingReply.new(user).due.map(&:id)
      reminder_ids = Reminder.accessible_to(user)
                             .pending
                             .where(due_at: ...1.day.from_now)
                             .where.not(source_type: "Task")
                             .order(:due_at)
                             .pluck(:id)
      task_ids = begin
        overdue      = Task.accessible_to(user).active.where.not(due_at: nil).where(due_at: ...Time.current)
        high_prio    = Task.accessible_to(user).active.where(priority: [ Task.priorities[:high], Task.priorities[:urgent] ])
        overdue.or(high_prio).distinct.pluck(:id)
      rescue StandardError
        []
      end
    else
      thread_ids = reminder_ids = task_ids = []
    end

    DigestMailer.needs_attention(
      user: user,
      thread_ids: thread_ids,
      reminder_ids: reminder_ids,
      task_ids: task_ids
    )
  end
end
