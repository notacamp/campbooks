# frozen_string_literal: true

module Api
  module App
    module Documents
      # Document detail, mutations, and file serving for the SPA. Mirrors the
      # logic in DocumentsController and Documents::SkimController but returns
      # JSON envelopes. All mutations return the updated document so the client
      # can patch its TanStack Query cache immediately (optimistic update support).
      class DocumentsController < Api::App::BaseController
        before_action :set_document, only: %i[
          show file update rename approve reject toggle_star
          reprocess settle unsettle push_to_drive push_to_notion push_to_zoho_drive
        ]

        # GET /api/app/documents/:id
        def show
          render_data(Api::App::DocumentSerializer.new(@document, detail: true).as_json)
        end

        # GET /api/app/documents/:id/file
        # Redirects to the raw file (inline or attachment). The SPA can render
        # PDFs in an iframe or open a download by reading the disposition param.
        def file
          blob = if params[:type] == "processed" && @document.processed_pdf.attached?
            @document.processed_pdf.blob
          else
            @document.original_file.blob
          end

          return render_error("no_file", "No file attached.", status: :not_found) if blob.nil?

          url = if params[:disposition] == "attachment"
            rails_storage_proxy_url(blob, host: request.base_url, disposition: "attachment")
          else
            rails_storage_proxy_url(blob, host: request.base_url)
          end

          redirect_to url, allow_other_host: true
        end

        # POST /api/app/documents
        # Accepts multipart form with files[]. Reuses DocumentsController#create logic.
        def create
          files = Array(params[:files]).reject(&:blank?)
          return render_error("no_files", "No files provided.", status: :unprocessable_entity) if files.empty?

          documents = files.map do |file|
            doc = ::Document.new(
              source: :manual_upload,
              ai_status: :pending,
              review_status: :pending,
              workspace: current_workspace
            )
            doc.original_file.attach(file)
            doc.save!
            ::DocumentProcessJob.perform_later(doc.id)
            doc
          end

          render_data(
            documents.map { |d| Api::App::DocumentSerializer.new(d).as_json },
            status: :created
          )
        end

        # PATCH /api/app/documents/:id
        def update
          incoming_meta = document_params.delete(:metadata)&.to_h || {}
          @document.assign_attributes(document_params)
          merge_metadata_fields(incoming_meta)

          if @document.save
            @document.generate_canonical_filename!
            render_data(Api::App::DocumentSerializer.new(@document, detail: true).as_json)
          else
            render_record_invalid(ActiveRecord::RecordInvalid.new(@document))
          end
        end

        # PATCH /api/app/documents/:id/rename
        def rename
          @document.assign_title(params.dig(:document, :title))
          @document.save!
          render_data({ id: @document.id, title: @document.display_title })
        end

        # POST /api/app/documents/:id/approve
        def approve
          @document.approve!(by: current_user)
          finalize_approval(@document)
          render_data(Api::App::DocumentSerializer.new(@document).as_json)
        end

        # POST /api/app/documents/:id/reject
        def reject
          @document.reject!
          ::Notifier.documents_need_review(@document.workspace, bump: false)
          render_data(Api::App::DocumentSerializer.new(@document).as_json)
        end

        # PATCH /api/app/documents/:id/toggle_star
        def toggle_star
          @document.update!(starred: !@document.starred?)
          render_data({ id: @document.id, starred: @document.starred? })
        end

        # POST /api/app/documents/:id/reprocess
        def reprocess
          was_failed = @document.ai_failed?
          @document.update!(ai_status: :pending, review_status: :pending,
                            ai_processing_attempts: 0, ai_error: nil,
                            reviewed_by: nil, reviewed_at: nil)
          ::Notifier.document_recovered(@document) if was_failed
          ::Notifier.documents_need_review(@document.workspace, bump: false)
          ::DocumentProcessJob.perform_later(@document.id)
          render_data({ id: @document.id, ai_status: "pending" })
        end

        # POST /api/app/documents/:id/settle
        def settle
          @document.mark_settled!
          render_data(Api::App::DocumentSerializer.new(@document).as_json)
        end

        # DELETE /api/app/documents/:id/settle
        def unsettle
          @document.mark_unsettled!
          render_data(Api::App::DocumentSerializer.new(@document).as_json)
        end

        # POST /api/app/documents/:id/push_to_drive
        def push_to_drive
          unless current_workspace.google_drive_accounts.connected.exists?
            return render_error("not_connected", "Google Drive is not connected.", status: :unprocessable_entity)
          end

          unless @document.classification&.google_drive_config
            return render_error("no_config", "No Drive config for this document type.", status: :unprocessable_entity)
          end

          ::GoogleDrivePushJob.perform_later(@document.id)
          render_data({ queued: true })
        end

        # POST /api/app/documents/:id/push_to_notion
        def push_to_notion
          mapping = @document.notion_database_mapping
          return render_error("no_mapping", "No Notion mapping configured.", status: :unprocessable_entity) unless mapping

          ::NotionPushJob.perform_later(@document.id, mapping.id)
          render_data({ queued: true })
        end

        # POST /api/app/documents/:id/push_to_zoho_drive
        def push_to_zoho_drive
          accounts = current_workspace.zoho_drive_accounts.active
          mapping  = ::DriveFolderMapping.where(zoho_drive_account: accounts)
                        .find_by(document_type_id: @document.document_type_id)
          mapping ||= ::DriveFolderMapping.where(zoho_drive_account: accounts)
                        .find_by(document_type_id: nil)

          return render_error("no_mapping", "No Zoho Drive mapping configured.", status: :unprocessable_entity) unless mapping
          return render_error("inactive_account", "Zoho Drive account is inactive.", status: :unprocessable_entity) unless mapping.zoho_drive_account.active?

          ::ZohoDriveUploadJob.perform_later(@document.id)
          render_data({ queued: true })
        end

        # POST /api/app/documents/reprocess_all
        def reprocess_all
          filters = ::Documents::Filters.from_params(params)
          documents = filters.apply(
            current_workspace.documents,
            workspace: current_workspace, user: current_user
          ).reprocessable

          count = documents.count
          documents.find_each do |doc|
            was_failed = doc.ai_failed?
            doc.update!(ai_status: :pending, review_status: :pending, ai_processing_attempts: 0,
                        ai_error: nil, reviewed_by: nil, reviewed_at: nil)
            ::Notifier.document_recovered(doc) if was_failed
            ::DocumentProcessJob.perform_later(doc.id)
          end
          ::Notifier.documents_need_review(current_workspace, bump: false)
          render_data({ queued: count })
        end

        # POST /api/app/documents/export
        def export
          filters = ::Documents::Filters.from_params(params)
          export_record = current_workspace.exports.create!(
            status: :pending,
            filters: filters.to_persistable_h(workspace: current_workspace, user: current_user)
          )
          ::ExportJob.perform_later(export_record.id)
          render_data({ export_id: export_record.id, status: "pending" }, status: :accepted)
        end

        # GET /api/app/documents/merge
        def merge
          ids = params[:ids].to_s.split(",").map(&:strip).reject(&:blank?).uniq
          docs = current_workspace.documents.includes(:classification).where(id: ids).order(:id)
          render_data(docs.map { |d| Api::App::DocumentSerializer.new(d, detail: true).as_json })
        end

        # POST /api/app/documents/perform_merge
        def perform_merge
          keep = current_workspace.documents.find(params[:keep_id])
          merge_ids = Array(params[:merge_ids]).map(&:to_s) - [ keep.id.to_s ]
          merged = 0

          merge_ids.each do |id|
            dup = current_workspace.documents.find_by(id: id)
            next unless dup

            dup.document_email_messages.find_each do |dem|
              keep.document_email_messages.find_or_create_by!(email_message_id: dem.email_message_id)
            end

            if keep.ai_extraction_data.blank? && dup.ai_extraction_data.present?
              merged_metadata = (dup.metadata || {}).merge(
                "vendor_name"    => (dup.vendor_name.presence    || keep.vendor_name),
                "client_name"    => (dup.client_name.presence    || keep.client_name),
                "invoice_number" => (dup.invoice_number.presence || keep.invoice_number),
                "document_date"  => ((dup.metadata || {})["document_date"] || (keep.metadata || {})["document_date"]),
                "amount_cents"   => (dup.amount_cents || keep.amount_cents)
              ).compact
              keep.update_columns(
                ai_extraction_data: dup.ai_extraction_data,
                ai_confidence_score: dup.ai_confidence_score,
                metadata: merged_metadata
              )
            end

            dup.document_email_messages.delete_all
            dup.original_file.purge if dup.original_file.attached?
            dup.processed_pdf.purge if dup.processed_pdf.attached?
            dup.delete
            merged += 1
          end

          render_data(Api::App::DocumentSerializer.new(keep, detail: true).as_json)
        end

        private

        def set_document
          @document = current_workspace.documents
                        .accessible_to(current_user)
                        .includes(:classification)
                        .find(params[:id])
        end

        def finalize_approval(document)
          ::Notifier.documents_need_review(document.workspace, bump: false)
          ::Documents::FinalizeApprovalJob.perform_later(document.id)
        end

        def merge_metadata_fields(incoming_meta)
          return if incoming_meta.blank?

          schema = ::DocumentTypes::Schema.for(@document.classification)
          current_meta = (@document.metadata || {}).dup
          incoming_meta.each do |k, v|
            field = schema.field(k)
            coerced = field ? field.coerce(v) : v.presence
            if coerced.nil?
              current_meta.delete(k.to_s)
            else
              current_meta[k.to_s] = coerced
            end
          end
          @document.metadata = current_meta
        end

        def document_params
          params.require(:document).permit(
            :document_type_id, :vendor_name, :vendor_nif, :document_date, :due_date,
            :invoice_number, :amount_cents, :currency, :buyer_nif, :tax_amount_cents,
            :tax_rate, :description, :expense_category, :company_vat_present,
            :client_name, :client_nif,
            :bank_name, :account_number, :period_start, :period_end,
            :opening_balance_cents, :closing_balance_cents,
            :receipt_number, :payment_method,
            metadata: {}
          )
        end
      end
    end
  end
end
