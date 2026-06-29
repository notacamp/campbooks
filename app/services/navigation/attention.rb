module Navigation
  # Decides the "action required" dot for each primary-nav section: true when a
  # section has resources the user hasn't viewed yet — unskimmed emails, unseen
  # feed items, unviewed pending reminders, unviewed docs needing review, unread
  # Scout replies.
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
      when :files     then new_documents?
      when :scout     then new_scout?
      else false
      end
    end

    # Active feed items (not dismissed/acted) the user hasn't seen yet.
    # seen_at is stamped when the item scrolls into view on the home feed.
    def new_feed?
      @user.feed_items.active.where(seen_at: nil).exists?
    end

    # Unviewed mail on readable accounts. viewed_at is stamped when the user
    # opens an email or processes it via Skim — regardless of which surface.
    def new_mail?
      EmailMessage.accessible_to(@user).where(viewed_at: nil).exists?
    end

    # Pending reminders the user hasn't viewed yet. viewed_at is stamped when
    # reminders appear on the Calendar or Reminders page.
    def new_calendar?
      Reminder.accessible_to(@user).pending.where(viewed_at: nil).exists?
    end

    # Documents needing review that the user hasn't viewed yet. viewed_at is
    # stamped when the user visits the Files index (where Skim now lives).
    def new_documents?
      return false unless @user.workspace

      @user.workspace.documents.needs_review.where(viewed_at: nil).exists?
    end

    # Unviewed AI messages on the user's scout-visible threads. viewed_at
    # is stamped when the user visits Scout.
    def new_scout?
      AgentMessage.where(agent_thread: @user.agent_threads.scout_visible,
                         author_type: :ai, viewed_at: nil).exists?
    end
  end
end
