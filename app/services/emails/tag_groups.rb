# frozen_string_literal: true

module Emails
  # The single source of truth for which inbox threads collapse out of the main
  # list into per-group rows. A group is any set of tags sharing a `group_name`
  # (the four built-in default groups — Notifications / Newsletters & promos /
  # Social / Updates — plus anything the user groups themselves). Replaces the old
  # category-driven Emails::SmartGroups.
  #
  # A thread collapses if ANY of its messages carries a tag in an enabled group,
  # and it surfaces in EVERY group it qualifies for (additive multi-membership).
  # It is NEVER collapsed — it stays inline — when a human clearly cares about it:
  # the owner replied (last_outbound_at), it is pinned, the sender is starred, or
  # any message is classified `important`. The same guards feed both the main-list
  # exclusion and the group rows/counts, so the numbers always agree. The caller
  # applies the exclusion on the inbox root only; folder and search views show
  # everything inline.
  class TagGroups
    def initialize(workspace, readable_account_ids)
      @workspace = workspace
      @readable_account_ids = readable_account_ids
    end

    # Guarded EmailThread relation of every grouped thread, for excluding them
    # from the main list. nil when the workspace has no grouped tags.
    def excluded_scope
      return @excluded_scope if defined?(@excluded_scope)

      ids = grouped_tag_ids
      return @excluded_scope = nil if ids.empty?

      @excluded_scope = guarded(threads_with_tags(ids))
    end

    # Guarded EmailThread relation for one group's drill-in view, or nil when the
    # group name matches no tags.
    def group_scope(group_name)
      ids = tag_ids_for_group(group_name)
      return nil if ids.empty?

      guarded(threads_with_tags(ids))
    end

    # The group's display color (its first tag's color), for the identity dot
    # next to the group name. nil when the group name matches no tags.
    def group_color(group_name)
      name = group_name.to_s
      grouped_tags.find { |t| t.group_name == name }&.color
    end

    # Row data for the collapsed group rows: one hash per non-empty group
    # ({ label:, count:, senders:, color: }). Counts are restricted to threads
    # with a message in an inbox folder so the number matches the drill-in view.
    def build_groups(inbox_folder_ids)
      grouped_tags_by_name.filter_map do |group_name, tags|
        scope = guarded(threads_with_tags(tags.map(&:id)))
        counted = scope.joins(:email_messages)
        counted = counted.where(email_messages: { provider_folder_id: inbox_folder_ids }) if inbox_folder_ids.present?
        count = counted.distinct.count
        next if count.zero?

        { label: group_name, count: count, senders: senders_for(scope), color: tags.first&.color }
      end
    end

    private

    # Visible, grouped tags for this workspace (the default bucket tags plus any
    # the user has grouped). Loaded once; the row/scope helpers slice this array.
    def grouped_tags
      @grouped_tags ||= Tag.where(workspace_id: @workspace&.id).visible.grouped.by_name.to_a
    end

    def grouped_tags_by_name
      grouped_tags.group_by(&:group_name)
    end

    def grouped_tag_ids
      grouped_tags.map(&:id)
    end

    def tag_ids_for_group(group_name)
      name = group_name.to_s
      grouped_tags.select { |t| t.group_name == name }.map(&:id)
    end

    # email_thread_ids with at least one message carrying one of `tag_ids`.
    def threads_with_tags(tag_ids)
      EmailMessage.where(email_account_id: @readable_account_ids)
                  .where.not(email_thread_id: nil)
                  .joins(:email_message_tags)
                  .where(email_message_tags: { tag_id: tag_ids })
                  .select(:email_thread_id)
    end

    # The "never collapse" guards — a human cares about this thread regardless of
    # its tags: the owner replied, it is pinned, the sender is starred, or a
    # message is important.
    def guarded(thread_id_subquery)
      EmailThread.where(email_account_id: @readable_account_ids)
                 .where(id: thread_id_subquery)
                 .where(last_outbound_at: nil)
                 .where.not(id: EmailThread.pinned)
                 .where.not(id: starred_sender_thread_ids)
                 .where.not(id: important_message_thread_ids)
    end

    def starred_sender_thread_ids
      EmailMessage.joins(:contact)
                  .where(email_account_id: @readable_account_ids)
                  .where.not(email_thread_id: nil)
                  .where.not(contacts: { starred_at: nil })
                  .select(:email_thread_id)
    end

    def important_message_thread_ids
      EmailMessage.where(email_account_id: @readable_account_ids)
                  .where.not(email_thread_id: nil)
                  .where(category: "important")
                  .select(:email_thread_id)
    end

    # Up to 3 distinct recent sender addresses for the row's avatar stack.
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
