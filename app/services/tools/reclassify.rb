module Tools
  class Reclassify
    def self.call(args = {})
      scope = EmailMessage.accessible_to(Current.user).where.not(status: :ignored)

      if args["email_ids"].present?
        scope = scope.where(id: args["email_ids"])
      else
        scope = scope.where(status: args["status"]) if args["status"].present?
        scope = scope.where(ai_priority: args["ai_priority"]) if args["ai_priority"].present?

        if args["tag_name"].present?
          scope = scope.joins(:tags).where(tags: { name: args["tag_name"].to_s.downcase.strip })
        end

        if args["date_from"].present?
          date = Date.parse(args["date_from"]) rescue nil
          scope = scope.where("received_at >= ?", date) if date
        end

        if args["date_to"].present?
          date = Date.parse(args["date_to"]) rescue nil
          scope = scope.where("received_at <= ?", date) if date
        end
      end

      count = 0
      scope.find_each do |message|
        Ai::EmailClassifier.new(message).classify!
        count += 1
      rescue => e
        Rails.logger.warn("[Reclassify] Failed for message #{message.id}: #{e.message}")
      end

      { reclassified_count: count }
    end
  end
end
