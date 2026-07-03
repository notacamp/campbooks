# frozen_string_literal: true

module Emails
  # The single source of truth for inbox smart groups: which threads are
  # collapsed out of the main inbox list into per-bucket group rows
  # (Notifications / Newsletters & promos / Social / Updates), driven by the
  # rules-based email category the triage ladder stamps on every message.
  #
  # A thread is bundled only when EVERY message's category sits in an enabled
  # noise bucket — any personal/important/uncategorized message keeps the whole
  # thread inline (fail-open), as does a reply from the user, a starred sender,
  # or a pin. Search, custom folders, and specific-folder views never bundle;
  # the caller applies this only on the inbox root.
  class SmartGroups
    BUCKETS = User::SMART_GROUP_BUCKETS

    def initialize(user, readable_account_ids)
      @user = user
      @readable_account_ids = readable_account_ids
    end

    def enabled?
      @user.smart_groups_enabled? && enabled_buckets.any?
    end

    def enabled_buckets
      @enabled_buckets ||= @user.enabled_smart_group_buckets
    end

    # EmailThread relation of every bundled thread (all enabled buckets), for
    # excluding them from the main list. nil when the feature is off.
    def bundled_scope
      return nil unless enabled?

      @bundled_scope ||= guarded(thread_ids_fully_in(enabled_buckets))
    end

    # EmailThread relation for one bucket's drill-in view. nil when the bucket
    # is disabled or unknown.
    def bundled_scope_for(bucket)
      bucket = bucket.to_s
      return nil unless BUCKETS.include?(bucket) && @user.smart_group_enabled?(bucket)

      guarded(thread_ids_fully_in([ bucket ]))
    end

    # Row data for the collapsed group rows: one hash per non-empty enabled
    # bucket, mirroring the tag-group chip shape (label/count/senders) plus
    # bucket/type so the row partial can branch. Counts are restricted to
    # threads with a message in an inbox folder so the number matches what the
    # drill-in view (which keeps the inbox folder filter) actually shows.
    def build_groups(inbox_folder_ids)
      return [] unless enabled?

      enabled_buckets.filter_map do |bucket|
        scope = bundled_scope_for(bucket)
        next unless scope

        counted = scope.joins(:email_messages)
        counted = counted.where(email_messages: { provider_folder_id: inbox_folder_ids }) if inbox_folder_ids.present?
        count = counted.distinct.count
        next if count.zero?

        { type: :smart, bucket: bucket, count: count, senders: senders_for(scope) }
      end
    end

    private

    # email_thread_ids where every message's category is in `buckets`.
    # `CASE WHEN category IN (...)` yields NULL for a NULL category, and COUNT
    # skips NULLs — so any uncategorized (or personal/important/unknown)
    # message breaks the equality and the thread stays inline.
    def thread_ids_fully_in(buckets)
      EmailMessage.where(email_account_id: @readable_account_ids)
                  .where.not(email_thread_id: nil)
                  .group(:email_thread_id)
                  .having(
                    "COUNT(*) = COUNT(CASE WHEN email_messages.category IN (?) THEN 1 END)",
                    buckets
                  )
                  .select(:email_thread_id)
    end

    # The "never bundle" guards: the user replied, pinned it, or starred the
    # sender — signals a human cares about this thread no matter its category.
    def guarded(thread_id_subquery)
      EmailThread.where(email_account_id: @readable_account_ids)
                 .where(id: thread_id_subquery)
                 .where(last_outbound_at: nil)
                 .where.not(id: EmailThread.pinned)
                 .where.not(id: starred_sender_thread_ids)
    end

    def starred_sender_thread_ids
      EmailMessage.joins(:contact)
                  .where(email_account_id: @readable_account_ids)
                  .where.not(email_thread_id: nil)
                  .where.not(contacts: { starred_at: nil })
                  .select(:email_thread_id)
    end

    # Up to 3 distinct recent sender addresses for the row's avatar stack —
    # same shape as EmailMessagesController#build_tag_groups senders.
    def senders_for(thread_scope)
      sender_rows = EmailMessage.where(email_thread_id: thread_scope.select(:id))
                                .order(received_at: :desc)
                                .limit(20)
                                .pluck(:from_address, :contact_id, :email_account_id)
      top_rows = sender_rows.uniq { |row| row[0] }.first(3)
      account_colors = EmailAccount.where(id: top_rows.map { |row| row[2] }.compact.uniq)
                                   .pluck(:id, :color).to_h
      top_rows.map do |address, contact_id, account_id|
        { email: address, contact_id: contact_id, sent: false, account_color: account_colors[account_id] }
      end
    end
  end
end
