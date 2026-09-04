# frozen_string_literal: true

module People
  # A thread as seen from the Person conversation pane: the EmailThread record
  # plus, when loaded, its accessible messages ordered oldest-first. Threads
  # that are not loaded (everything but the newest thread on the first page)
  # carry only what their heading needs — the message count and the newest
  # message's time and id — and fetch their messages lazily through
  # People::ThreadsController when they scroll into view. The unit of rendering
  # for Campbooks::People::ThreadBlock.
  ConversationThread = Data.define(:thread, :messages, :count, :latest_at, :newest_id) do
    def initialize(thread:, messages: nil, count: nil, latest_at: nil, newest_id: nil)
      count ||= messages ? messages.size : 0
      latest_at ||= messages&.last&.received_at
      newest_id ||= messages&.last&.id
      super(thread:, messages:, count:, latest_at:, newest_id:)
    end

    # Whether the messages are in memory (false for a lazily-loaded thread).
    def loaded?
      !messages.nil?
    end

    # Human-readable subject, falling back to the thread's own column.
    def subject
      thread.display_subject
    end

    # The newest message in this thread (nil until loaded).
    def newest
      messages&.last
    end

    # All messages except the newest (shown folded / closed).
    def older
      loaded? ? messages[0...-1] : []
    end

    # The most recent inbound (not-sent) message: the natural reply target. When
    # the newest message in this thread is outbound, we still want to target the
    # last thing they sent us, not our own message. Nil when the thread has no
    # inbound messages or is not loaded.
    def reply_target
      messages&.reverse_each&.find { |m| !m.sent? }
    end
  end
end
