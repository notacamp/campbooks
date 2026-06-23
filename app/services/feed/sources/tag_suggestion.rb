module Feed
  module Sources
    # Filing suggestions: Scout proposes a tag and no reply is expected. Rendered
    # as a compact one-tap "file it" chip-row. Excludes anything with a
    # draft_reply suggestion so reply-worthy mail stays an action/reminder.
    class TagSuggestion < Feed::Source
      def self.key = "tag_suggestion"

      def candidates
        collapse_by_thread(
          base_scope.reorder(:id).find_each.filter_map do |m|
            tag_name = suggested_tag_name(m)
            next if tag_name.blank?
            {
              subject: m,
              dedupe_key: "tag_suggestion:#{m.id}",
              sort_at: m.received_at || m.created_at,
              score: 0,
              attention: false,
              data: { tag_name: tag_name }
            }
          end
        )
      end

      def still_valid?(item, m)
        return false if m.nil? || m.skimmed_at.present? || m.ai_todo_dismissed?
        return false unless in_inbox?(m)
        tag_name = item.data["tag_name"].to_s
        return false if tag_name.blank?
        # Valid only while Scout still suggests it and it isn't already applied.
        suggested_tag_name(m) == tag_name && m.tags.none? { |t| t.name.to_s.casecmp?(tag_name) }
      end

      private

      def base_scope
        in_inbox(admitted(
          EmailMessage.accessible_to(user)
            .where("ai_suggested_actions @> ?", add_tag_json)
            .where.not("ai_suggested_actions @> ?", draft_reply_json)
            .where(ai_todo_dismissed: false, skimmed_at: nil)
            .select(:id, :received_at, :created_at, :ai_suggested_actions, :email_account_id, :email_thread_id)
        ))
      end

      def suggested_tag_name(m)
        action = Array(m.ai_suggested_actions).find { |a| a["tool"] == "add_tag" }
        action&.dig("args", "tag_name").to_s.presence
      end

      def add_tag_json = [ { tool: "add_tag" } ].to_json
      def draft_reply_json = [ { tool: "draft_reply" } ].to_json
    end
  end
end
