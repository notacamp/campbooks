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

  # Renders the daily "needs attention" digest with all three sections visible.
  # Uses the seed admin user; seeds sample follow-up threads, reminders, and
  # tasks into the preview DB if the live data doesn't cover all sections.
  # Demo data only — no real names/emails.
  def needs_attention_digest
    user = User.find_by(email_address: "admin@example.com") || User.first
    return DigestMailer.needs_attention(user: User.new(name: "Demo", email_address: "demo@example.com")) unless user

    ws      = user.workspace
    account = user.readable_email_accounts.first

    # ── Follow-ups section ───────────────────────────────────────────────────
    thread_ids = Emails::AwaitingReply.new(user).due.map(&:id)
    if thread_ids.empty? && account
      t1 = EmailThread.find_or_create_by!(email_account: account, subject: "[Preview] Q3 budget proposal") do |t|
        t.last_outbound_at = 5.days.ago
        t.last_inbound_at  = 6.days.ago
        t.follow_up_reason = "Waiting for budget sign-off"
      end
      EmailMessage.find_or_create_by!(email_account: account, email_thread: t1, provider_message_id: "preview-fu-1") do |m|
        m.from_address = "partner@example.com"
        m.to_address   = user.email_address
        m.subject      = "[Preview] Q3 budget proposal"
        m.provider_folder_id = "INBOX"
        m.received_at  = 6.days.ago
        m.status       = :processed
        m.read         = true
      end
      t2 = EmailThread.find_or_create_by!(email_account: account, subject: "[Preview] Contract renewal — Acme") do |t|
        t.last_outbound_at = 9.days.ago
        t.last_inbound_at  = 10.days.ago
      end
      EmailMessage.find_or_create_by!(email_account: account, email_thread: t2, provider_message_id: "preview-fu-2") do |m|
        m.from_address = "vendor@example.com"
        m.to_address   = user.email_address
        m.subject      = "[Preview] Contract renewal — Acme"
        m.provider_folder_id = "INBOX"
        m.received_at  = 10.days.ago
        m.status       = :processed
        m.read         = true
      end
      thread_ids = [ t1.id, t2.id ]
    end

    # ── Reminders section ────────────────────────────────────────────────────
    reminder_ids = Reminder.accessible_to(user)
                           .pending
                           .where(due_at: ...1.day.from_now)
                           .where.not(source_type: "Task")
                           .order(:due_at)
                           .pluck(:id)
    if reminder_ids.empty? && ws
      src_thread = EmailThread.find_or_create_by!(email_account: account, subject: "[Preview] Invoice #1042") do |t|
        t.last_inbound_at = 3.days.ago
      end
      src_msg = EmailMessage.find_or_create_by!(email_account: account, email_thread: src_thread, provider_message_id: "preview-rem-src") do |m|
        m.from_address = "billing@vendor.example.com"
        m.to_address   = user.email_address
        m.subject      = "[Preview] Invoice #1042"
        m.provider_folder_id = "INBOX"
        m.received_at  = 3.days.ago
        m.status       = :processed
        m.read         = true
      end
      r1 = Reminder.find_or_create_by!(workspace: ws, source: src_msg, reminder_type: :payment_due) do |r|
        r.title      = "[Preview] Invoice #1042 — payment due"
        r.due_at     = 4.hours.from_now
        r.status     = :pending
        r.confidence = 0.95
      end
      r2 = Reminder.find_or_create_by!(workspace: ws, source: src_msg, reminder_type: :renewal) do |r|
        r.title      = "[Preview] Insurance renewal overdue"
        r.due_at     = 1.day.ago
        r.status     = :pending
        r.confidence = 0.90
      end
      reminder_ids = [ r2.id, r1.id ]
    end

    # ── Tasks section ────────────────────────────────────────────────────────
    overdue_tasks   = Task.accessible_to(user).active.where.not(due_at: nil).where(due_at: ...Time.current)
    high_prio_tasks = Task.accessible_to(user).active.where(priority: [ Task.priorities[:high], Task.priorities[:urgent] ])
    task_ids = overdue_tasks.or(high_prio_tasks).distinct.pluck(:id)
    if task_ids.empty? && ws
      tk1 = Task.find_or_create_by!(workspace: ws, title: "[Preview] Prepare investor deck") do |t|
        t.created_by = user
        t.status     = :todo
        t.priority   = :high
        t.due_at     = 2.days.ago
        t.confidence = 1.0
      end
      tk2 = Task.find_or_create_by!(workspace: ws, title: "[Preview] Review partnership agreement") do |t|
        t.created_by = user
        t.status     = :in_progress
        t.priority   = :urgent
        t.confidence = 1.0
      end
      task_ids = [ tk1.id, tk2.id ]
    end

    DigestMailer.needs_attention(
      user: user,
      thread_ids: thread_ids,
      reminder_ids: reminder_ids,
      task_ids: task_ids
    )
  end
end
