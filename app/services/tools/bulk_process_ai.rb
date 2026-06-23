module Tools
  class BulkProcessAi
    def self.call(email_ids:)
      base = EmailMessage.accessible_to(Current.user)
      messages = base.where(id: email_ids)
      thread_ids = messages.where.not(email_thread_id: nil).pluck(:email_thread_id).uniq
      scope = base.where(email_thread_id: thread_ids)

      count = 0
      scope.find_each do |message|
        message.update!(status: :fetched)
        EmailProcessJob.perform_later(message.id)
        count += 1
      end

      { count: count }
    end
  end
end
