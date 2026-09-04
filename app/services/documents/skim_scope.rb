# frozen_string_literal: true

module Documents
  # The single source of truth for "which documents Skim should review" for a
  # given workspace member: every AI-completed document awaiting human sign-off
  # (review_status: pending), minus anything the user rejected as junk, filtered
  # to only the documents accessible to that user (FolderAccessible).
  #
  # The user parameter is REQUIRED — omitting it (nil) fails closed to
  # Document.none so callers cannot accidentally expose cross-user data.
  #
  # Used by both Documents::SkimController (show/tray) and
  # Documents::SkimTrayBroadcaster so the tray and viewer always agree.
  #
  # Ordered most-uncertain-first (lowest ai_confidence_score; never-scored docs —
  # NULL — sort first) so the diciest calls surface at the top of the stack.
  class SkimScope
    # Safety ceiling so a pathological review backlog can't bloat the DOM.
    MAX = 200

    def self.for(workspace, user)
      new(workspace, user).relation
    end

    def initialize(workspace, user)
      @workspace = workspace
      @user = user
    end

    def relation
      return Document.none unless @workspace && @user

      @workspace.documents
                .needs_review
                .reviewable_attachment
                .where(id: Document.accessible_to(@user))
                .includes(:classification)
                .with_attached_original_file
                .order(Arel.sql("ai_confidence_score ASC NULLS FIRST, created_at ASC"))
                .limit(MAX)
    end
  end
end
