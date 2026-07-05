# frozen_string_literal: true

module Digests
  module Sources
    # Gathers EmailMessage records matching the configured query within the lookback
    # period. Uses Emails::Search#scope (keyword/filter path) so it is embedding-
    # free and job-safe. Permission is enforced by EmailMessage.accessible_to(user)
    # inside Emails::Search, which scopes to the user's readable_email_accounts.
    class Emails < Base
      def self.direction = :lookback

      def items(period)
        query = source_config["query"].to_s

        scope = ::Emails::Search.new(user: user, params: { q: query }, folder_ids: nil).scope
        # Emails::Search date filters are day-granular (beginning/end_of_day);
        # digests need exact boundaries or consecutive issues overlap.
        scope = scope.where(received_at: period.begin..period.end).limit(MAX_ITEMS)

        scope.map do |msg|
          Digests::Item.new(
            source_type: "email",
            source_id:   msg.id,
            title:       msg.subject.to_s.presence || I18n.t("digests.no_subject"),
            subtitle:    sender_label(msg),
            summary:     truncate(msg.ai_summary.presence || plain_body_preview(msg)),
            timestamp:   msg.received_at&.iso8601
          )
        end
      end

      private

      def sender_label(msg)
        msg.from_address.to_s.strip
      end

      # First slice of the body as plain text: bodies are HTML, and tag soup
      # would pollute the AI prompt. Slice before parsing to bound Loofah's work.
      def plain_body_preview(msg)
        html = msg.body.to_s[0, 20_000]
        return "" if html.blank?

        Loofah.fragment(html).text(encode_special_chars: false)
      end
    end
  end
end
