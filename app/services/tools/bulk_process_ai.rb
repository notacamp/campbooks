module Tools
  # Bulk-triggers AI processing (re-analysis + embedding) for a set of emails.
  #
  # == Anti-drain cap (MAX_BULK_AI)
  #
  # One bulk POST can expand to every message in the selected threads, which on a
  # large selection blows up the job queue and hammers the AI provider. A hard cap
  # limits the fan-out per request; emails beyond the cap are skipped and the caller
  # receives `capped: true` so it can surface a clear message to the user.
  #
  # Enqueued jobs run at BACKGROUND_PRIORITY so they yield to user-triggered
  # and real-time ingest work.
  class BulkProcessAi
    # Hard ceiling on the number of EmailProcessJobs enqueued per bulk request.
    # Beyond this count, processing is skipped and the response reports `capped: true`.
    MAX_BULK_AI = 200

    def self.call(email_ids:)
      base = EmailMessage.accessible_to(Current.user)
      messages = base.where(id: email_ids)
      thread_ids = messages.where.not(email_thread_id: nil).pluck(:email_thread_id).uniq
      scope = base.where(email_thread_id: thread_ids)

      total_count = scope.count
      capped = total_count > MAX_BULK_AI

      count = 0
      scope.limit(MAX_BULK_AI).find_each do |message|
        message.update!(status: :fetched)
        EmailProcessJob.set(priority: ApplicationJob::BACKGROUND_PRIORITY).perform_later(message.id)
        count += 1
      end

      if capped
        Rails.logger.info(
          "[Tools::BulkProcessAi] Capped at #{MAX_BULK_AI} / #{total_count} messages for workspace #{Current.workspace&.id}"
        )
      end

      { count: count, total_count: total_count, capped: capped }
    end
  end
end
