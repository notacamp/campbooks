module Feed
  # Base class for a feed source: one domain that contributes cards to the feed.
  #
  # A source turns its slice of the user's world (actionable emails, documents
  # needing review, …) into candidate attribute hashes that Feed::Generator
  # upserts into `feed_items`. It also answers #still_valid? so the reader can
  # drop a materialized card whose underlying record has since been handled,
  # before a periodic refresh reconciles it.
  #
  # Adding a kind to the feed = subclass this, define `self.key`, `#candidates`
  # and `#still_valid?`, and register the class in `Feed::Source.all`.
  class Source
    # The registry. ORDER MATTERS: Feed::Generator keeps the first source that
    # claims a given subject, so an aged "needs a reply" email becomes a quiet
    # reminder rather than a hero action, and a filing-only email becomes a tag
    # suggestion. Most-specific framing first.
    def self.all
      [
        Feed::Sources::CalendarEvent,
        Feed::Sources::Reminder,
        Feed::Sources::StarredEmail,
        Feed::Sources::FollowUp,
        Feed::Sources::ReplyReminder,
        Feed::Sources::TagSuggestion,
        Feed::Sources::EmailAction
      ]
    end

    # Source class that owns a given kind string (for read-time #still_valid?).
    def self.for_kind(kind)
      all.find { |klass| klass.key == kind.to_s }
    end

    # The kind string this source produces (one of FeedItem::KINDS). Override.
    def self.key = raise NotImplementedError

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    def key = self.class.key

    # ⇒ Array of attribute hashes, permission-scoped to @user:
    #   { subject:, dedupe_key:, sort_at:, score:, attention:, data: }
    # The generator stamps `kind` from #key and fills in user/workspace.
    def candidates = raise NotImplementedError

    # Is a materialized item still worth showing? Re-checks the live record.
    # `subject` is nil when the record was deleted ⇒ return false.
    def still_valid?(item, subject) = raise NotImplementedError

    private

    attr_reader :user, :now

    # Collapse email candidates to one per thread (keeping the most recent
    # message) so a noisy thread doesn't spray near-identical cards, and stamp the
    # thread's message count into `data` for a disambiguating chip. Threadless
    # messages pass through untouched. One grouped COUNT query, no N+1.
    def collapse_by_thread(candidates)
      threaded, loose = candidates.partition { |c| c[:subject].email_thread_id }
      return candidates if threaded.empty?

      counts = EmailMessage.where(email_thread_id: threaded.map { |c| c[:subject].email_thread_id })
                           .group(:email_thread_id).count

      collapsed = threaded
        .group_by { |c| c[:subject].email_thread_id }
        .map do |thread_id, group|
          winner = group.max_by { |c| c[:subject].received_at || Time.at(0) }
          winner.merge(data: (winner[:data] || {}).merge("thread_count" => counts[thread_id].to_i))
        end

      collapsed + loose
    end

    # Filter an EmailMessage scope to senders admitted into the feed. Always drops
    # blocked senders. In whitelist mode, keeps only allowed/starred senders —
    # unknown/undecided senders belong in Skim's Pending bucket, not the feed.
    # Filters on contact_id via WHERE, so the scope's SELECT need not include it.
    def admitted(scope)
      if whitelist_mode?
        ids = admitted_contact_ids
        ids.empty? ? scope.none : scope.where(contact_id: ids)
      else
        ids = blocked_contact_ids
        return scope if ids.empty?

        scope.where("email_messages.contact_id IS NULL OR email_messages.contact_id NOT IN (?)", ids)
      end
    end

    # Inbox-folder gate, the lockstep partner of Emails::SkimScope: archiving (or
    # filing into any non-inbox folder) rewrites provider_folder_id, so an
    # inbox-only scope drops that mail from feed candidates. Reconcile then
    # resolves any card already materialized for it. NOTE: deliberately NOT for
    # snooze-due reminders — Tools::Snooze moves the thread to a Snoozed folder,
    # so gating that path would suppress every snooze nudge.
    def in_inbox(scope)
      Emails::InboxFolders.constrain(scope, feed_accounts)
    end

    # Read-time partner of #in_inbox so Feed::Reader drops an archived card on the
    # next render, before the generator reconciles it. Fails open when folder ids
    # can't be resolved (matches the scope filter, which then applies no constraint).
    def in_inbox?(message)
      return true if inbox_folder_ids.empty?

      inbox_folder_ids.include?(message.provider_folder_id)
    end

    def feed_accounts
      @feed_accounts ||= user.readable_email_accounts.to_a
    end

    def inbox_folder_ids
      @inbox_folder_ids ||= Emails::InboxFolders.ids_for(feed_accounts)
    end

    # Per-message admission check, for sources that iterate records rather than a
    # filterable scope (e.g. the snooze-due branch of ReplyReminder).
    def admitted_message?(message)
      cid = message.contact_id
      if whitelist_mode?
        cid.present? && admitted_contact_ids.include?(cid)
      else
        cid.nil? || blocked_contact_ids.exclude?(cid)
      end
    end

    def whitelist_mode?
      return @whitelist_mode if defined?(@whitelist_mode)

      @whitelist_mode = user.workspace&.whitelist_mode? || false
    end

    def blocked_contact_ids
      @blocked_contact_ids ||= Contact.where(workspace_id: user.workspace_id).blocked.pluck(:id)
    end

    # Allowed OR starred — both reach the inbox in whitelist mode.
    def admitted_contact_ids
      @admitted_contact_ids ||= Contact.where(workspace_id: user.workspace_id)
        .where("list_status = ? OR starred_at IS NOT NULL", Contact.list_statuses[:allowed]).pluck(:id)
    end
  end
end
