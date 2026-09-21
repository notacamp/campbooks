# frozen_string_literal: true

module Api
  module App
    module DocumentSkim
      # Document Skim ring-deck for the SPA. Mirrors Documents::SkimController
      # but returns JSON envelopes. The deck state is stateless (rebuilt on every
      # request from the workspace review queue); no cursor needed because one
      # document = one card.
      class SkimController < Api::App::BaseController
        before_action :set_document, only: %i[approve reclassify update_fields reprocess dismiss restore]

        # GET /api/app/document_skim
        def show
          begin
            ::Documents::PendingAnalysisCatchUp.run(current_workspace)
          rescue StandardError => e
            Rails.logger.warn("[api/document_skim] catch-up failed: #{e.class}: #{e.message}")
          end

          rings = current_rings
          render_data(Api::App::DocumentSkimDeckSerializer.new(rings).as_json)
        end

        # GET /api/app/document_skim/tray
        def tray
          render_data(Api::App::DocumentSkimDeckSerializer.new(current_rings).as_json)
        end

        # POST /api/app/document_skim/:id/approve
        def approve
          @document.approve!(by: current_user)
          ::Notifier.documents_need_review(@document.workspace, bump: false)
          ::Documents::FinalizeApprovalJob.set(wait: 7.seconds).perform_later(@document.id)
          render_data({ ok: true, action: "approved", next_deck: Api::App::DocumentSkimDeckSerializer.new(current_rings).as_json })
        end

        # PATCH /api/app/document_skim/:id/reclassify
        def reclassify
          type = current_workspace.document_types.find(params[:document_type_id])
          @document.reclassify!(type, by: current_user)
          ::Notifier.documents_need_review(@document.workspace, bump: false)
          ::Documents::FinalizeApprovalJob.set(wait: 7.seconds).perform_later(@document.id)
          render_data({
            ok: true,
            action: "reclassified",
            document_type_id: type.id,
            type_label: type.name.humanize,
            next_deck: Api::App::DocumentSkimDeckSerializer.new(current_rings).as_json
          })
        end

        # PATCH /api/app/document_skim/:id/update_fields
        def update_fields
          @document.assign_attributes(field_params)
          @document.assign_title(params.dig(:document, :title)) if params[:document]&.key?(:title)
          merge_metadata
          @document.save!
          @document.generate_canonical_filename!
          render_data({ ok: true, action: "updated", display_title: @document.display_title })
        end

        # POST /api/app/document_skim/:id/reprocess
        def reprocess
          @document.update!(ai_status: :pending, review_status: :pending,
                            ai_processing_attempts: 0, ai_error: nil,
                            reviewed_by: nil, reviewed_at: nil)
          ::DocumentProcessJob.perform_later(@document.id)
          ::Notifier.documents_need_review(@document.workspace, bump: false)
          render_data({ ok: true, action: "reprocessing", next_deck: Api::App::DocumentSkimDeckSerializer.new(current_rings).as_json })
        end

        # POST /api/app/document_skim/:id/dismiss
        def dismiss
          @document.reject!
          ::Notifier.documents_need_review(@document.workspace, bump: false)
          render_data({ ok: true, action: "dismissed", next_deck: Api::App::DocumentSkimDeckSerializer.new(current_rings).as_json })
        end

        # POST /api/app/document_skim/:id/restore
        def restore
          @document.restore!
          ::Notifier.documents_need_review(@document.workspace, bump: false)
          render_data({ ok: true, action: "restored", next_deck: Api::App::DocumentSkimDeckSerializer.new(current_rings).as_json })
        end

        private

        def set_document
          @document = current_workspace.documents.find(params[:id])
        end

        def current_rings
          ::Documents::SkimBuilder.new(
            ::Documents::SkimScope.for(current_workspace, current_user)
          ).rings
        end

        def field_params
          params.require(:document).permit(:description, *::Document.extracted_field_names)
        end

        def merge_metadata
          raw = params.dig(:document, :metadata)
          return if raw.blank?

          schema      = ::DocumentTypes::Schema.for(@document.classification)
          permit_keys = (schema.fields.map(&:key) + ::Document.extracted_field_names).uniq
          incoming    = raw.permit(*permit_keys, :title).to_h.transform_values { |v| v.to_s.strip.presence }
          return if incoming.blank?

          updated = (@document.metadata || {}).dup
          incoming.each do |key, value|
            if value.nil?
              updated.delete(key)
            elsif (field = schema.field(key))
              coerced = field.coerce(value)
              coerced.nil? ? updated.delete(key) : (updated[key] = coerced)
            else
              updated[key] = value
            end
          end
          @document.metadata = updated
        end
      end
    end
  end
end
