# frozen_string_literal: true

module Documents
  # Pushes a freshly-built document Skim tray to a workspace's Turbo Stream so the
  # /documents ring tray stays live — category ring counts drop as documents are
  # approved / reclassified / dismissed without a manual reload. Mirrors
  # Emails::SkimTrayBroadcaster; the index subscribes via
  # turbo_stream_from "doc_skim_#{workspace.id}".
  #
  # Replaces the inner #doc_skim_tray_content (not the turbo-permanent #doc_skim_tray
  # frame itself), so the stable lazy frame is preserved while its contents refresh.
  class SkimTrayBroadcaster
    def self.refresh(workspace)
      new(workspace).refresh
    end

    def initialize(workspace)
      @workspace = workspace
    end

    def refresh
      return unless @workspace

      rings = Documents::SkimBuilder.new(Documents::SkimScope.for(@workspace)).rings
      html = ApplicationController.render(partial: "documents/skim/tray_content", locals: { rings: rings })

      Turbo::StreamsChannel.broadcast_replace_to(
        "doc_skim_#{@workspace.id}",
        target: "doc_skim_tray_content",
        html: html
      )
    rescue => e
      Rails.logger.error("[Documents::SkimTrayBroadcaster] #{e.class}: #{e.message}")
    end
  end
end
