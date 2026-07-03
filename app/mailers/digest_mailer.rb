class DigestMailer < ApplicationMailer
  # Threads listed in the body before we collapse the rest into "and N more".
  MAX_ITEMS = 8

  # The daily "waiting on replies" digest: conversations the owner sent last and is
  # still waiting to hear back on (Emails::AwaitingReply#due, resolved by the job).
  # Reloads the threads by id (so a thread deleted since the job enqueued just drops
  # out) and sends nothing if none survive. Links back into the inbox to act.
  def waiting_on_replies(user:, thread_ids:)
    @user = user
    threads = EmailThread.where(id: thread_ids)
                         .includes(:email_account, :email_messages)
                         .sort_by { |thread| thread.last_outbound_at || Time.at(0) }
    return if threads.empty?

    with_recipient_locale(user) do
      @count = threads.size
      @items = threads.first(MAX_ITEMS).map { |thread| item_for(thread) }
      @more = @count - @items.size
      @inbox_url = email_messages_url
      @settings_url = settings_notifications_url
      mail(to: user.email_address, subject: t(".subject", count: @count))
    end
  end

  private

  def item_for(thread)
    message = thread.latest_message
    {
      subject: thread.display_subject,
      days: thread.last_outbound_at ? ((Time.current - thread.last_outbound_at) / 1.day).floor : 0,
      reason: thread.follow_up_reason.presence,
      url: message ? email_message_url(message) : email_messages_url
    }
  end
end
