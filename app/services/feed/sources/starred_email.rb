module Feed
  module Sources
    # Mail from starred senders, promoted to the top of the feed. Each message is
    # its own card (deliberately NOT collapse_by_thread — a thread still collapses
    # via the Generator's per-thread claim, but distinct emails never group with
    # other senders, since a starred-sender email outweighs a group of ordinary
    # mail). Placed first in Feed::Source.all so it claims these before the generic
    # sources — a starred sender surfaces even without a Scout action prompt.
    class StarredEmail < Feed::Source
      WINDOW = 21.days

      def self.key = "starred_email"

      def candidates
        base_scope.reorder(received_at: :desc).find_each.map do |m|
          {
            subject: m,
            dedupe_key: "starred_email:#{m.id}",
            sort_at: m.received_at || m.created_at,
            score: score_for(m),
            # Pin only mail that still needs the user: unread, or urgent by
            # Scout's read. Once read, the card yields the attention cluster
            # and lives high in the timeline off the starred-contact boost.
            attention: !m.read? || m.ai_priority == "high",
            data: { starred_sender: true }
          }
        end
      end

      def still_valid?(_item, m)
        return false if m.nil? || m.skimmed_at.present?
        return false unless in_inbox?(m)

        m.contact&.starred?
      end

      private

      def base_scope
        in_inbox(
          EmailMessage.accessible_to(user)
            .joins(:contact).where.not(contacts: { starred_at: nil })
            .where(skimmed_at: nil)
            .where("email_messages.received_at >= ?", WINDOW.ago)
            .select(:id, :received_at, :created_at, :ai_priority, :read,
                    :category, :email_account_id, :email_thread_id, :contact_id,
                    :subject, :from_address) # the Generator's conversation claim
        )
      end

      # High intrinsic base; Feed::Ranking's starred-contact boost stacks on top,
      # so fresh starred mail leads the attention cluster while still decaying
      # with age like everything else.
      def score_for(m)
        s = 70
        s += 10 if m.ai_priority == "high"
        s += 5 unless m.read?
        s
      end
    end
  end
end
