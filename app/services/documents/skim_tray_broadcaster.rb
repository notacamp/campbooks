# frozen_string_literal: true

module Documents
  # Pushes a freshly-built document Skim tray to each workspace member's
  # per-user Turbo Stream so the ring tray stays live — category ring counts
  # drop as documents are approved / reclassified / dismissed without a manual
  # reload.
  #
  # Each member receives only the documents they are allowed to see
  # (Documents::SkimScope filters via Document.accessible_to). Broadcasting
  # per-user (doc_skim_user_<user_id>) prevents cross-user data exposure that
  # would otherwise occur when restricted-folder documents appeared in every
  # workspace member's tray.
  #
  # Replaces the inner #doc_skim_tray_content (not the turbo-permanent
  # #doc_skim_tray frame itself), so the stable lazy frame is preserved while
  # its contents refresh.
  class SkimTrayBroadcaster
    def self.refresh(workspace)
      new(workspace).refresh
    end

    def initialize(workspace)
      @workspace = workspace
    end

    def refresh
      return unless @workspace

      @workspace.users.find_each do |user|
        rings = Documents::SkimBuilder.new(Documents::SkimScope.for(@workspace, user)).rings
        html  = ApplicationController.render(partial: "documents/skim/tray_content", locals: { rings: rings })
        Turbo::StreamsChannel.broadcast_replace_to(
          "doc_skim_user_#{user.id}",
          target: "doc_skim_tray_content",
          html:   html
        )
      end
    rescue => e
      Rails.logger.error("[Documents::SkimTrayBroadcaster] #{e.class}: #{e.message}")
    end
  end
end
