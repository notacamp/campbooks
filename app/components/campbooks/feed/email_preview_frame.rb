# frozen_string_literal: true

module Campbooks
  module Feed
    # The server half of ExpandablePreview: the matching turbo-frame carrying the
    # email body (sandboxed EmailHtmlPreview, or the clamped text fallback), or a
    # quiet note when the message is gone or no longer accessible. Always renders
    # the frame — a frameless response would surface Turbo's "Content missing"
    # inside the card. `subject` is the resolved EmailMessage (may be nil).
    class EmailPreviewFrame < Campbooks::Feed::Base
      register_element :turbo_frame

      def view_template
        turbo_frame(id: "feed_item_#{item.id}_preview", class: "block") do
          if subject
            email_body_preview(subject, margin: "mt-2")
          else
            p(class: "mt-2 text-[13px] text-muted-foreground") { t(".unavailable") }
          end
        end
      end
    end
  end
end
