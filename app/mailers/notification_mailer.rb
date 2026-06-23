class NotificationMailer < ApplicationMailer
  # A teammate @mentioned the recipient in an email discussion thread.
  def mention(recipient:, actor_name:, subject_label:, snippet:, link_url:)
    assign(recipient, actor_name, subject_label, snippet, link_url)
    with_recipient_locale(recipient) do
      mail(to: recipient.email_address, subject: t(".subject", actor: actor_name))
    end
  end

  # New activity on a discussion thread the recipient follows.
  def thread_activity(recipient:, actor_name:, subject_label:, snippet:, link_url:)
    assign(recipient, actor_name, subject_label, snippet, link_url)
    with_recipient_locale(recipient) do
      mail(to: recipient.email_address, subject: t(".subject", subject: subject_label.to_s.truncate(60)))
    end
  end

  private

  def assign(recipient, actor_name, subject_label, snippet, link_url)
    @recipient = recipient
    @actor_name = actor_name
    @subject_label = subject_label
    @snippet = snippet
    @url = absolute_url(link_url)
  end

  # Notifier builds relative paths (no request context); turn them into absolute
  # URLs using the mailer's configured host.
  def absolute_url(link_url)
    return link_url if link_url.to_s.start_with?("http")
    "#{root_url.chomp('/')}#{link_url}"
  end
end
