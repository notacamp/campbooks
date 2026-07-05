# frozen_string_literal: true

module Feed
  module Sources
    # Digest issues on the home feed. A generated issue with show_in_feed enabled
    # surfaces as a feed card for up to 3 days, letting the user skim the digest
    # from the home screen without opening the issue page.
    class DigestIssue < Feed::Source
      WINDOW = 3.days
      SCORE  = 60

      def self.key = "digest_issue"

      def candidates
        return [] unless Features.digests?

        ::DigestIssue
          .joins(:scheduled_digest)
          .preload(:scheduled_digest)
          .where(user_id: user.id, status: ::DigestIssue.statuses[:generated])
          .where(scheduled_digests: { enabled: true, show_in_feed: true })
          .where("digest_issues.created_at >= ?", now - WINDOW)
          .order(created_at: :desc)
          .map do |issue|
            {
              subject:    issue,
              dedupe_key: "digest_issue:#{issue.id}",
              sort_at:    issue.created_at,
              score:      SCORE,
              attention:  false,
              data:       {
                "digest_name" => issue.scheduled_digest.name,
                "overview"    => issue.overview,
                "item_count"  => issue.item_count
              }
            }
          end
      end

      def still_valid?(_item, subject)
        subject.present? && subject.status_generated?
      end
    end
  end
end
