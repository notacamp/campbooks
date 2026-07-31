# frozen_string_literal: true

module Api
  module V1
    # Serializes a DraftEmail — the composer's autosaved, unsent state. Drafts are
    # private to their author, so a token only ever sees the acting user's own.
    class DraftSerializer
      def initialize(draft)
        @draft = draft
      end

      def as_json(*)
        {
          id: @draft.id,
          mode: @draft.mode,
          to: @draft.to_address,
          cc: @draft.cc_address,
          bcc: @draft.bcc_address,
          subject: @draft.subject,
          body: @draft.body,
          quoted_body: @draft.quoted_body,
          signature_id: @draft.signature_id,
          email_account_id: @draft.email_account_id,
          in_reply_to_id: @draft.in_reply_to_id,
          dismissed: @draft.dismissed_at.present?,
          display_title: @draft.display_title,
          attachments: @draft.attachment_entries,
          created_at: @draft.created_at&.iso8601,
          updated_at: @draft.updated_at&.iso8601
        }
      end
    end
  end
end
