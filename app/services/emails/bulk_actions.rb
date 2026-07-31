# frozen_string_literal: true

module Emails
  # Shared engine for bulk email/thread actions. Both the web bulk toolbar
  # (EmailMessages::BulkController) and the REST API (Api::V1::EmailBulkController)
  # dispatch through here, so selection expansion (explicit ids + smart-group
  # names → thread-expanded message ids), tool dispatch, and the live-inbox
  # broadcast are defined once. Rendering — Turbo streams vs the JSON envelope —
  # stays in each controller.
  #
  # Every underlying Tools::Bulk* service already scopes to
  # EmailMessage.accessible_to(Current.user), so callers on any surface get the
  # same permission boundary for free.
  class BulkActions
    # Tools this engine can dispatch. The forward composer is intentionally absent
    # — it never calls a Tools::Bulk* service, it just opens a compose dock.
    TOOLS = %w[archive unarchive mark_read mark_unread move_to_folder tag delete
               process_ai scout_chat snooze unsnooze].freeze

    Result = Struct.new(:tool, :result, :selected_ids, :all_ids, keyword_init: true) do
      # The tool ran and produced a result (a nil result means it failed or was a
      # no-op — e.g. move_to_folder with no folder given).
      def ok? = !result.nil?

      # Distinct from a failed run: nothing was selected to act on at all.
      def empty_selection? = selected_ids.empty?

      # A tool (currently only `tag`) can surface a soft error as { error: ... }
      # rather than nil; callers should treat this as a failure with a message.
      def error_message = result.is_a?(Hash) ? result[:error].presence : nil
    end

    def self.call(**kwargs) = new(**kwargs).call

    # options carries the tool-specific parameters that used to be read off the
    # controller's params: folder_name / folder_id (move_to_folder),
    # tag_name / tag_action (tag), snoozed_until (snooze).
    def initialize(tool:, user:, email_ids: [], groups: [], options: {}, broadcast: true)
      @tool = tool.to_s
      @user = user
      @email_ids = email_ids
      @groups = groups
      @options = (options || {}).to_h.with_indifferent_access
      @broadcast = broadcast
    end

    def call
      ids = clean(@email_ids)
      ids = (ids + expand_groups(clean(@groups))).uniq
      return Result.new(tool: @tool, result: nil, selected_ids: [], all_ids: []) if ids.empty?

      all_ids = expand_to_threads(ids)
      result = dispatch(all_ids, ids)
      broadcast_inbox_bulk(all_ids) if result && @broadcast
      Result.new(tool: @tool, result: result, selected_ids: ids, all_ids: all_ids)
    end

    private

    attr_reader :user, :options

    def clean(list)
      Array(list).map(&:to_s).reject(&:blank?).uniq
    end

    def dispatch(all_ids, selected_ids)
      case @tool
      when "archive"
        Tools::BulkArchive.call("email_ids" => all_ids)
      when "unarchive"
        Tools::BulkUnarchive.call("email_ids" => all_ids)
      when "mark_read"
        Tools::BulkMarkRead.call(email_ids: selected_ids, read: true)
      when "mark_unread"
        Tools::BulkMarkRead.call(email_ids: selected_ids, read: false)
      when "move_to_folder"
        folder_name = options[:folder_name].presence
        folder_id = options[:folder_id].presence
        return nil unless folder_name || folder_id
        Tools::BulkMoveToFolder.call(email_ids: selected_ids, folder_id: folder_id, folder_name: folder_name)
      when "tag"
        action = options[:tag_action].presence || "add"
        Tools::BulkTag.call("email_ids" => all_ids, "tag_name" => options[:tag_name], "action" => action)
      when "delete"
        Tools::BulkDelete.call(email_ids: selected_ids)
      when "process_ai"
        Tools::BulkProcessAi.call(email_ids: selected_ids)
      when "scout_chat"
        Tools::BulkScoutChat.call(email_ids: selected_ids, user: user)
      when "snooze"
        snoozed_until = options[:snoozed_until].presence
        return nil unless snoozed_until
        Tools::BulkSnooze.call("email_ids" => all_ids, "snoozed_until" => snoozed_until)
      when "unsnooze"
        Tools::BulkUnsnooze.call("email_ids" => all_ids)
      end
    end

    # Expand each selected message id to every message in its thread, so a
    # thread-scoped action hits the whole conversation. Gated to readable mail.
    def expand_to_threads(email_ids)
      base = EmailMessage.accessible_to(user)
      thread_ids = base.where(id: email_ids).where.not(email_thread_id: nil).pluck(:email_thread_id).uniq
      base.where(email_thread_id: thread_ids).pluck(:id)
    end

    # Resolve each smart-group name to the inbox message ids that belong to it,
    # using the same guarded TagGroups scope as the drill-in view so the
    # permission boundary is identical regardless of which tool is called.
    def expand_groups(group_names)
      return [] if group_names.empty?

      accounts = user.readable_email_accounts.to_a
      tag_groups = Emails::TagGroups.new(user.workspace, accounts.map(&:id))

      group_names.flat_map do |group_name|
        threads = tag_groups.group_scope(group_name)
        next [] unless threads

        messages = EmailMessage.where(email_thread_id: threads.select(:id))
        Emails::InboxFolders.constrain(messages, accounts).pluck(:id).map(&:to_s)
      end.uniq
    end

    # Push the bulk change to every reader's open inbox, one broadcast per
    # affected thread. Tools that don't change the inbox list (forward,
    # process_ai, scout_chat) are skipped.
    INBOX_BULK_REMOVE  = %w[archive snooze delete move_to_folder].freeze
    INBOX_BULK_UPSERT  = %w[unarchive unsnooze].freeze
    INBOX_BULK_REPLACE = %w[mark_read mark_unread tag].freeze

    def broadcast_inbox_bulk(message_ids)
      kind = if INBOX_BULK_REMOVE.include?(@tool) then :remove
      elsif INBOX_BULK_UPSERT.include?(@tool) then :upsert
      elsif INBOX_BULK_REPLACE.include?(@tool) then :replace
      end
      return unless kind

      thread_ids = EmailMessage.where(id: message_ids).where.not(email_thread_id: nil).distinct.pluck(:email_thread_id)
      EmailThread.where(id: thread_ids).find_each { |thread| Emails::InboxBroadcaster.public_send(kind, thread) }
    rescue => e
      Rails.logger.error("[Emails::BulkActions] inbox broadcast failed for #{@tool}: #{e.class}: #{e.message}")
    end
  end
end
