# frozen_string_literal: true

module People
  # A thread as seen from the Person conversation pane: the EmailThread record +
  # the accessible messages ordered oldest-first. Used as the unit of rendering
  # for Campbooks::People::ThreadBlock.
  ConversationThread = Data.define(:thread, :messages) do
    # Human-readable subject, falling back to the thread's own column.
    def subject
      thread.display_subject
    end

    # The newest message in this thread.
    def newest
      messages.last
    end

    # All messages except the newest (shown folded / closed).
    def older
      messages[0...-1]
    end

    # Total accessible message count in this thread.
    def count
      messages.size
    end

    # When the most recent accessible message arrived.
    def latest_at
      newest&.received_at
    end

    # The most recent inbound (not-sent) message: the natural reply target. When
    # the newest message in this thread is outbound, we still want to target the
    # last thing they sent us, not our own message. Returns nil when the thread
    # has no inbound messages.
    def reply_target
      messages.reverse_each.find { |m| !m.sent? }
    end
  end
end
