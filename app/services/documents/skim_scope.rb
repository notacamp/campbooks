# frozen_string_literal: true

module Documents
  # The single source of truth for "which documents Skim should review" for a
  # workspace: every AI-completed document awaiting human sign-off (review_status:
  # pending), minus anything the user rejected as junk. Used by both Documents::SkimController
  # (show/tray) and Documents::SkimTrayBroadcaster so the tray and viewer always agree.
  #
  # Ordered most-uncertain-first (lowest ai_confidence_score; never-scored docs —
  # NULL — sort first) so the diciest calls surface at the top of the stack.
  class SkimScope
    # Safety ceiling so a pathological review backlog can't bloat the DOM.
    MAX = 200

    def self.for(workspace)
      new(workspace).relation
    end

    def initialize(workspace)
      @workspace = workspace
    end

    def relation
      return Document.none unless @workspace

      @workspace.documents
                .needs_review
                .reviewable_attachment
                .includes(:classification)
                .with_attached_original_file
                .order(Arel.sql("ai_confidence_score ASC NULLS FIRST, created_at ASC"))
                .limit(MAX)
    end
  end
end
