class EmailThreadsController < ApplicationController
  before_action :require_authentication

  # Threads are read through the email-message reader. These routes exist for
  # links that point at a thread directly (e.g. the dashboard activity feed),
  # so redirect to the thread's latest message instead of 500-ing.
  def index
    redirect_to email_messages_path
  end

  def show
    thread = EmailThread.find(params[:id])
    # A thread the user can't access 404s rather than redirecting — the find above
    # is intentionally global, so a redirect would reveal that an out-of-reach
    # thread exists. Fail closed and indistinguishable from "no such thread".
    raise ActiveRecord::RecordNotFound unless thread.accessible_by?(Current.user)

    latest = thread.latest_message

    # Mailbox users get the full email reader; teammates pulled in by a mention
    # (no mailbox access) get the focused discussion view.
    if latest && thread.email_account&.accessible_by?(Current.user)
      redirect_to email_message_path(latest, folder_id: latest.provider_folder_id)
    else
      @thread = thread
      @agent_thread = thread.agent_thread
      @comments = @agent_thread&.agent_messages&.chronological || []
      @messages = thread.email_messages.order(received_at: :asc)
      @latest_message = latest
      render :show
    end
  end
end
