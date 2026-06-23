class DashboardController < ApplicationController
  def show
    readable_ids = Current.user.readable_email_accounts.pluck(:id)
    readable_emails = EmailMessage.where(email_account_id: readable_ids)

    # Stuck processing alert
    @stuck_documents = Current.workspace.documents.where(ai_status: :processing)
                            .where("created_at < ?", 1.hour.ago).count

    # ── Section 1: Unified action items ──────────────────────

    todo_emails = readable_emails.with_ai_todos.includes(:email_account).map do |e|
      { kind: :email, record: e, time: e.received_at, priority: priority_for(e) }
    end

    review_docs = Current.workspace.documents
                         .needs_review
                         .order(created_at: :desc).limit(8)
                         .map { |d| { kind: :document, record: d, time: d.created_at, priority: 2 } }

    pending = AgentMessage.needing_reply
                     .joins("INNER JOIN agent_threads ON agent_threads.id = agent_messages.agent_thread_id")
                     .joins("INNER JOIN email_threads ON email_threads.id = agent_threads.contextable_id AND agent_threads.contextable_type = 'EmailThread'")
                     .where(agent_threads: { purpose: :email_chat })
                     .where(email_threads: { email_account_id: readable_ids })
                     .includes(agent_thread: :contextable)
                     .order(created_at: :asc).limit(5)
                     .map { |c| { kind: :reply, record: c, time: c.created_at, priority: 3 } }

    @action_items = (todo_emails + review_docs + pending)
                      .sort_by { |i| [ i[:priority], -i[:time].to_i ] }
                      .take(8)

    @has_action_items = @action_items.any?

    # ── Section 2: Recent activity ───────────────────────────

    recent_docs = Current.workspace.documents
                       .order(created_at: :desc).limit(10)
                       .map { |d| { kind: :document, record: d, time: d.created_at } }

    recent_emails = readable_emails
                      .order(received_at: :desc).limit(10)
                      .map { |e| { kind: :email, record: e, time: e.received_at } }

    @recent_scans = EmailScanLog.where(email_account_id: readable_ids)
                      .where.not(completed_at: nil)
                      .where("emails_processed > 0 OR documents_created > 0")
                      .order(completed_at: :desc).limit(3)

    @recent_activity = (recent_docs + recent_emails)
                         .sort_by { |item| item[:time] }
                         .reverse
                         .take(6)
  end

  private

  def priority_for(email)
    case email.ai_priority
    when "high" then 0
    when "medium" then 1
    else 2
    end
  end
end
