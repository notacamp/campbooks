module Navigation
  # Decides the "action required" dot for each primary-nav section: true when a
  # section has content newer than the user last looked at it
  # (User#seen_section_at), cleared on visit by the TracksSectionVisit concern.
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

    def since(section)
      @user.seen_section_at(section)
    end

    # Active feed items (not dismissed/acted) materialized since the last visit.
    def new_feed?
      @user.feed_items.active.where("feed_items.created_at > ?", since(:home)).exists?
    end

    # Mail that arrived (on a readable account) since the last inbox visit.
    def new_mail?
      EmailMessage.accessible_to(@user)
                  .where("email_messages.received_at > ?", since(:mail))
                  .exists?
    end

    # New events OR new pending reminders since the last calendar visit. Reminders
    # have no nav item of their own — they ride inside the Calendar dot.
    def new_calendar?
      ts = since(:calendar)
      CalendarEvent.accessible_to(@user).visible
                   .where("calendar_events.created_at > ?", ts).exists? ||
        Reminder.accessible_to(@user).pending
                .where("reminders.created_at > ?", ts).exists?
    end

    # Workspace documents (shared across the workspace) added since the last visit.
    def new_documents?
      return false unless @user.workspace

      @user.workspace.documents.where("documents.created_at > ?", since(:documents)).exists?
    end

    # New Scout replies since the last visit — AI-authored messages on the user's
    # own visible threads (proactive briefings aren't persisted, so they don't dot).
    def new_scout?
      AgentMessage.where(agent_thread: @user.agent_threads.scout_visible, author_type: :ai)
                  .where("agent_messages.created_at > ?", since(:scout))
                  .exists?
    end
  end
end
