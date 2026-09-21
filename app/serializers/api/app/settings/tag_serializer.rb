# frozen_string_literal: true

module Api
  module App
    module Settings
      # Tag as shown in inbox settings.
      class TagSerializer
        def initialize(tag, message_count: nil)
          @tag = tag
          @message_count = message_count
        end

        def as_json(*)
          {
            id: @tag.id,
            name: @tag.name,
            color: @tag.color,
            hidden: @tag.hidden,
            kind: @tag.kind,
            source: @tag.source,
            group_name: @tag.group_name,
            prompt: @tag.prompt,
            system_label: @tag.system_label,
            external: @tag.external?,
            message_count: @message_count,
            created_at: @tag.created_at
          }
        end
      end
    end
  end
end
