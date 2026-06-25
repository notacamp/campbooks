# frozen_string_literal: true

module Emails
  # Pushes live inbox thread-row changes to every user who can read the mailbox,
  # so the inbox list reflects email CRUD in real time — new mail appears, and
  # archive/snooze/trash/pin/tag/read done in any tab, on any device, or by a
  # teammate sharing the account all reflect without a manual reload.
  #
  # Mirrors the app's per-user stream pattern (e.g. Emails::SkimTrayBroadcaster,
  # "sync_status_#{user.id}"). The inbox subscribes via two streams:
  #
  #   "inbox_#{user.id}"      — every inbox view subscribes. Carries the
  #                             idempotent, filter-SAFE ops: #replace (refresh a
  #                             row in place) and #remove (drop a row). Both target
  #                             a row by its DOM id, so they no-op when the row
  #                             isn't on the recipient's page (search mode, the
  #                             wrong folder, the board) — they can never insert a
  #                             row where it doesn't belong.
  #   "inbox_feed_#{user.id}" — ONLY the unfiltered default-inbox view subscribes
  #                             (see show.html.erb / index.html.erb). Carries the
  #                             filter-SENSITIVE insert: a remove-then-prepend that
  #                             floats a new/restored thread to the top of
  #                             #email_threads. Folder/group/search views simply
  #                             don't subscribe, so they never get a mis-inserted row.
  #
  # The split is what makes this correct with zero client-side JS: removals and
  # in-place refreshes go everywhere; inserts go only where the default inbox is
  # actually shown. Every operation is idempotent, and #upsert removes before it
  # prepends, so double-delivery (the acting tab's own request response racing the
  # broadcast) converges to exactly one row.
  class InboxBroadcaster
    include ActionView::RecordIdentifier # bare dom_id(thread, :thread_item)

    THREADS_CONTAINER = "email_threads"

    # A new or floated-to-top thread (new mail, unarchive, unsnooze). Refreshes the
    # row in place wherever it's shown, and floats it to the top of the default
    # inbox — but only when the thread actually belongs in an inbox folder, so a
    # sent-only or archived thread never gets injected into the inbox list.
    def self.upsert(thread) = new(thread).upsert

    # A thread that has left the inbox view (archive, trash, snooze, block).
    def self.remove(thread) = new(thread).remove

    # An in-place row refresh (pin/unpin, tag add/remove, read/unread) — no move.
    def self.replace(thread) = new(thread).replace

    def initialize(thread)
      @thread = thread
    end

    def remove
      return unless @thread
      target = dom_id(@thread, :thread_item)
      each_user do |user|
        Turbo::StreamsChannel.broadcast_remove_to(stream(user), target: target)
      end
    rescue => e
      log(e)
    end

    def replace
      reload!
      return if @thread&.latest_message.nil?

      target = dom_id(@thread, :thread_item)
      each_user do |user|
        Turbo::StreamsChannel.broadcast_replace_to(stream(user), target: target, html: row_html(user))
      end
    rescue => e
      log(e)
    end

    def upsert
      reload!
      return if @thread&.latest_message.nil?
      # Only a thread that actually belongs in an inbox folder floats into the inbox
      # — a sent-only or still-archived thread is never injected (a reload wouldn't
      # show it there either).
      return unless inbox_thread?

      each_user do |user|
        # A single prepend is the idempotent "float to top, or insert". Turbo's
        # prepend de-duplicates by id (it removes any existing #email_threads child
        # carrying this row's id before inserting), so a thread already in the list
        # moves to the top with fresh content (new count/subject) and a brand-new one
        # is inserted — with no duplicate. We deliberately do NOT pair it with an
        # explicit remove: the two could be rendered out of order and the stray
        # remove would delete the row the prepend just inserted.
        Turbo::StreamsChannel.broadcast_prepend_to(feed_stream(user), target: THREADS_CONTAINER, html: row_html(user))
      end
    rescue => e
      log(e)
    end

    private

    def stream(user) = "inbox_#{user.id}"
    def feed_stream(user) = "inbox_feed_#{user.id}"

    # Reload with the preloads the row partial needs, so rendering adds no N+1:
    # per-message tags (subject/label chips) and attachments (the paperclip count),
    # plus each message's account (the avatar's account color).
    def reload!
      return unless @thread

      @thread = EmailThread.includes(
        :email_account,
        email_messages: [ :tags, :files_attachments, :email_account ]
      ).find_by(id: @thread.id) || @thread
    end

    # Every user allowed to READ the mailbox — owner + shared viewers/editors — i.e.
    # exactly who the inbox's `readable_email_accounts` scope would surface this
    # thread to (which merges `can_read: true`). Fails closed on a threadless/
    # account-less record.
    def each_user
      return unless @thread&.email_account_id

      EmailAccountUser.where(email_account_id: @thread.email_account_id, can_read: true)
                      .includes(:user)
                      .filter_map(&:user)
                      .uniq
                      .each { |user| yield user }
    end

    # True when the thread still has a message in one of its account's inbox
    # folders — the gate for the default-inbox prepend (keeps sent-only / archived
    # threads out of the live inbox list).
    def inbox_thread?
      inbox_ids = Emails::InboxFolders.ids_for([ @thread.email_account ])
      return false if inbox_ids.blank?

      @thread.email_messages.any? { |m| inbox_ids.include?(m.provider_folder_id) }
    end

    # The row HTML is identical for all recipients except for locale, so render
    # once per distinct locale. Rendered through a real view context (helpers,
    # components) with `layout: false`, exactly like the Skim tray broadcaster;
    # active: false because a broadcast can't know which recipient has the thread open.
    def row_html(user)
      (@row_html_by_locale ||= {})[locale_for(user)] ||=
        I18n.with_locale(locale_for(user)) do
          ApplicationController.render(
            partial: "email_messages/thread_row",
            locals: { thread: @thread, active: false },
            layout: false
          )
        end
    end

    def locale_for(user)
      user.locale.presence || I18n.default_locale
    end

    def log(error)
      Rails.logger.error("[Emails::InboxBroadcaster] #{error.class}: #{error.message}")
    end
  end
end
