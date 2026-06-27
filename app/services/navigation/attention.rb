module Navigation
  # Decides the "action required" dot for each primary-nav section: true when a
  # section has resources that need action or are new — unskimmed emails, pending
  # reminders, documents awaiting review, unread Scout replies, active feed items.
  #
  # One cheap EXISTS per section, memoized for the request. Fails closed — a nil
  # user lights no dots — mirroring the *.accessible_to permission gates.
  #
  #   Navigation::Attention.new(current_user).dot?(:mail) # => true / false
  class Attention
    def initialize(user)
      @user = user
      @cache = {}
    end

    def dot?(section)
      return false unless @user

      @cache.fetch(section) { @cache[section] = compute(section) }
    end

    private

    def compute(section)
      case section.to_sym
      when :home      then new_feed?
      when :mail      then new_mail?
      when :calendar  then new_calendar?
      when :documents then new_documents?
      when :scout     then new_scout?
      else false
      end
    end

    # Active feed items — not dismissed, not acted-on. The home dot lights up
    # whenever there's something in the feed to process.
    def new_feed?
      @user.feed_items.active.exists?
    end

    # Unskimmed mail on readable accounts. "Skimmed" means the user processed
    # the email via Skim or a feed action — regardless of which surface they
    # used. The dot clears naturally when the database state changes.
    def new_mail?
      EmailMessage.accessible_to(@user).where(skimmed_at: nil).exists?
    end

    # Pending reminders that need a decision (confirm / dismiss / snooze).
    # Calendar events are a view, not action items — reminders are the things
    # that actually need human attention.
    def new_calendar?
      Reminder.accessible_to(@user).pending.exists?
    end

    # Documents awaiting human sign-off (AI completed, review pending).
    def new_documents?
      return false unless @user.workspace

      @user.workspace.documents.needs_review.exists?
    end

    # Unread AI messages on the user's scout-visible threads. Marked read
    # when the user visits Scout; new replies arrive as unread, lighting the
    # dot back up.
    def new_scout?
      AgentMessage.where(agent_thread: @user.agent_threads.scout_visible,
                         author_type: :ai, read: false).exists?
    end
  end
end
