# frozen_string_literal: true

module Emails
  # Marks every message of a thread read (and viewed) the way opening it in the
  # inbox does: locally at once, at the provider through MarkReadJob, and live in
  # every open inbox through the InboxBroadcaster. Shared by the classic thread
  # view and the People conversation, so the two can never disagree.
  #
  # Returns true when at least one message was unread (so callers can refresh
  # derived state such as a People standing's unread dot), false otherwise.
  # Costs a single query when there is nothing left to mark.
  class MarkThreadRead
    def self.call(thread, account_id: nil)
      return false unless thread

      messages = thread.email_messages
      pending = messages.where(read: false).or(messages.where(viewed_at: nil))
                        .pluck(:id, :read, :provider_message_id)
      return false if pending.empty?

      messages.where(id: pending.map(&:first))
              .update_all(read: true, viewed_at: Time.current, updated_at: Time.current)
      unread_ids = pending.filter_map { |_id, read, provider_id| provider_id unless read }
      return false if unread_ids.empty?

      account_id ||= thread.email_account_id
      MarkReadJob.perform_later(account_id, unread_ids) if account_id
      Emails::InboxBroadcaster.replace(thread)
      true
    end
  end
end
