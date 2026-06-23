# frozen_string_literal: true

module Documents
  # Skim for Documents: the review queue (low-confidence AI classifications)
  # grouped into category "rings" and reviewed one document at a time,
  # Instagram-stories style — the document-world clone of SkimController.
  #
  # Categorisation is the document's own DocumentType category; there is no
  # clustering (every document is verified individually). Mutating actions are
  # workspace-scoped and return JSON for the doc-skim-mode controller's
  # fire-and-forget fetches; an Undo on the toast reverses approve/dismiss.
  class SkimController < ApplicationController
    before_action :set_document, only: %i[approve reclassify update_fields reprocess dismiss restore]

    def show
      # Opening Skim kicks off analysis for any document still waiting on AI, so the
      # review queue fills in rather than silently omitting un-analyzed uploads.
      # Best-effort: a catch-up hiccup must never block the queue from rendering.
      begin
        Documents::PendingAnalysisCatchUp.run(Current.workspace)
      rescue StandardError => e
        Rails.logger.warn("[skim] pending-analysis catch-up failed: #{e.class}: #{e.message}")
      end

      @rings = current_rings
      @start_category = params[:start].presence
      @document_types = Current.workspace.document_types.order(:category, :name)
      @standalone = !turbo_frame_request?

      # In the overlay (a turbo-frame request) skip the app chrome so only the
      # doc_skim_content frame is returned (no duplicate frame ids).
      render layout: false if turbo_frame_request?
    end

    # The /documents ring tray, lazily loaded into a turbo-frame so it never blocks
    # the index render. Same rings as the viewer, so a ring deep-links into its story.
    def tray
      @rings = current_rings
      render layout: false
    end

    # Approve the AI's classification. The Drive auto-push is DEFERRED ~7s (via
    # FinalizeApprovalJob, which self-guards on auto_push? and still-approved) so the
    # viewer's Undo can cancel it before it fires.
    def approve
      @document.approve!(by: current_user)
      Notifier.documents_need_review(@document.workspace, bump: false)
      Documents::FinalizeApprovalJob.set(wait: 7.seconds).perform_later(@document.id)
      refresh_tray
      render json: { ok: true, action: "approved" }
    end

    # Re-file under a different type, picked from the workspace's DocumentTypes.
    # Re-filing signs the document off, so this mirrors #approve: drop the review
    # badge and defer the same guarded Drive push (now under the *new* type).
    def reclassify
      type = Current.workspace.document_types.find(params[:document_type_id])
      @document.reclassify!(type, by: current_user)
      Notifier.documents_need_review(@document.workspace, bump: false)
      Documents::FinalizeApprovalJob.set(wait: 7.seconds).perform_later(@document.id)
      refresh_tray
      render json: { ok: true, action: "reclassified", document_type_id: type.id, type_label: type.name.humanize }
    end

    # Inline field fixes (the display name + the AI-extracted data). Status is
    # unchanged, so the ring structure doesn't move — no tray refresh. The name
    # (display title) and the extracted fields both live in metadata, so they're
    # applied separately from the column fields.
    def update_fields
      @document.assign_attributes(field_params)
      @document.assign_title(params.dig(:document, :title)) if params[:document]&.key?(:title)
      merge_metadata
      @document.save!
      @document.generate_canonical_filename!
      render json: { ok: true, action: "updated", display_title: @document.display_title }
    end

    def reprocess
      return if require_ai_provider!(:documents)

      @document.update!(ai_status: :pending, review_status: :pending, ai_processing_attempts: 0,
                        ai_error: nil, reviewed_by: nil, reviewed_at: nil)
      DocumentProcessJob.perform_later(@document.id)
      Notifier.documents_need_review(@document.workspace, bump: false)
      refresh_tray
      render json: { ok: true, action: "reprocessing" }
    end

    # Flag as junk / not-a-doc — drops out of the review feed (reversible via #restore).
    def dismiss
      @document.reject!
      Notifier.documents_need_review(@document.workspace, bump: false)
      refresh_tray
      render json: { ok: true, action: "dismissed" }
    end

    def restore
      @document.restore!
      Notifier.documents_need_review(@document.workspace, bump: false)
      refresh_tray
      render json: { ok: true, action: "restored" }
    end

    private

    def set_document
      @document = Current.workspace.documents.find(params[:id])
    end

    def current_rings
      Documents::SkimBuilder.new(Documents::SkimScope.for(Current.workspace)).rings
    end

    def refresh_tray
      Documents::SkimTrayBroadcaster.refresh(Current.workspace)
    end

    # The inline editor writes the same typed columns the detail page does — the
    # full per-type field set (Documents::ExtractedFieldSet::COLUMN_KEYS) plus the
    # free-text description. A blank enum picker means "unset" → nil, since assigning
    # "" to an enum column raises.
    def field_params
      permitted = params.require(:document).permit(:description, *Documents::ExtractedFieldSet::COLUMN_KEYS)
      Documents::ExtractedFieldSet::ENUM_KEYS.each { |key| permitted[key] = nil if permitted[key].blank? }
      permitted
    end

    # Inline edits to the AI-extracted fields write straight into the metadata hash
    # (the canonical store the detail page edits too). Merge — never replace — so the
    # display title and any keys not shown on the card survive; a blank clears a key.
    def merge_metadata
      raw = params.dig(:document, :metadata)
      return if raw.blank?

      # Tightened from permit!: only the AI-extracted field keys (+ display title)
      # may be merged into the JSONB metadata. They map to the same field set the
      # detail/skim forms render, so nothing legitimate is dropped.
      incoming = raw.permit(*Documents::ExtractedFieldSet::COLUMN_KEYS, :title)
                    .to_h.transform_values { |v| v.to_s.strip.presence }
      @document.metadata = (@document.metadata || {}).merge(incoming)
    end
  end
end
