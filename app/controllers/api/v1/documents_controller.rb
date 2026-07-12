# frozen_string_literal: true

module Api
  module V1
    # Public API for documents. Documents are workspace-scoped (every member of a
    # workspace sees them all), so access is gated through Current.workspace.
    class DocumentsController < BaseController
      before_action -> { doorkeeper_authorize! :"documents:read" },  only: [ :index, :show, :file ]
      before_action -> { doorkeeper_authorize! :"documents:write" }, only: [ :create, :update, :approve, :reject, :reclassify ]
      before_action :set_document, only: [ :show, :file, :update, :approve, :reject, :reclassify ]

      def index
        scope = apply_filters(Current.workspace.documents.recent)
        @pagy, documents = pagy(scope, limit: per_page)
        render_page(documents.map { |document| DocumentSerializer.new(document).as_json }, @pagy)
      end

      def show
        AuditEvent.log("document_read", user: Current.user, request: request, target: @document, via: "api")
        render_data(DocumentSerializer.new(@document, detail: true).as_json)
      end

      # Streams the original uploaded file (fetch with the same bearer token).
      def file
        unless @document.original_file.attached?
          return render_api_error("no_file", "This document has no attached file.", status: :not_found)
        end

        blob = @document.original_file
        send_data blob.download, filename: blob.filename.to_s, type: blob.content_type, disposition: "attachment"
      end

      # Upload one or more files. AI classification/extraction runs asynchronously,
      # so documents come back with ai_status "pending" and a 202.
      def create
        files = Array(params[:files]).reject(&:blank?)
        if files.empty?
          return render_api_error("missing_parameter", "files[] is required.", status: :bad_request)
        end

        documents = files.map do |file|
          document = Current.workspace.documents.new(
            source: :manual_upload, ai_status: :pending, review_status: :pending
          )
          document.original_file.attach(file)
          document.save!
          DocumentProcessJob.perform_later(document.id)
          document
        end

        render json: { data: documents.map { |document| DocumentSerializer.new(document).as_json } },
               status: :accepted
      end

      # Edit extracted fields. Does NOT change review state (use approve/reclassify
      # for that) — keeps the API explicit, unlike the web form's implicit approve.
      def update
        safe_params = document_params
        # Merge the metadata payload instead of replacing it: metadata= assignment
        # inside update! would wipe what the field accessor writers (vendor_name=, …)
        # just wrote AND any unsubmitted keys such as metadata["title"].
        incoming_meta = safe_params.delete(:metadata)&.to_h || {}
        @document.assign_attributes(safe_params)
        merge_metadata_payload(incoming_meta)
        @document.save!
        @document.generate_canonical_filename!
        render_data(DocumentSerializer.new(@document.reload, detail: true).as_json)
      end

      def approve
        @document.approve!(by: current_user)
        Notifier.documents_need_review(@document.workspace, bump: false)
        Documents::FinalizeApprovalJob.perform_later(@document.id)
        render_data(DocumentSerializer.new(@document.reload, detail: true).as_json)
      end

      def reject
        @document.reject!
        Notifier.documents_need_review(@document.workspace, bump: false)
        render_data(DocumentSerializer.new(@document.reload, detail: true).as_json)
      end

      # Change the document type; reclassifying also signs the document off
      # (approved), mirroring the app.
      def reclassify
        type = Current.workspace.document_types.find(params[:document_type_id])
        @document.reclassify!(type, by: current_user)
        Notifier.documents_need_review(@document.workspace, bump: false)
        Documents::FinalizeApprovalJob.perform_later(@document.id)
        render_data(DocumentSerializer.new(@document.reload, detail: true).as_json)
      end

      private

      def set_document
        @document = Current.workspace.documents.find(params[:id])
      end

      # Same merge semantics as the web DocumentsController#update: per-key schema
      # coercion, blank values remove the key, unsubmitted keys survive.
      def merge_metadata_payload(incoming_meta)
        return if incoming_meta.blank?

        schema       = DocumentTypes::Schema.for(@document.classification)
        current_meta = (@document.metadata || {}).dup
        incoming_meta.each do |k, v|
          field   = schema.field(k)
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
        params.permit(
          :document_type_id, :vendor_name, :vendor_nif, :document_date, :due_date,
          :invoice_number, :amount_cents, :currency, :buyer_nif, :tax_amount_cents,
          :tax_rate, :description, :expense_category, :company_vat_present,
          :client_name, :client_nif, :bank_name, :account_number, :period_start,
          :period_end, :opening_balance_cents, :closing_balance_cents,
          :receipt_number, :payment_method, metadata: {}
        )
      end

      def apply_filters(scope)
        scope = scope.where(document_type_id: params[:type]) if params[:type].present?

        if params[:review_status].present? && Document.review_statuses.key?(params[:review_status])
          scope = scope.by_review_status(params[:review_status])
        end

        if params[:ai_status].present? && Document.ai_statuses.key?(params[:ai_status])
          scope = scope.by_ai_status(params[:ai_status])
        end

        scope
      end

      def per_page
        requested = params[:per_page].to_i
        requested = 25 if requested <= 0
        [ requested, 100 ].min
      end
    end
  end
end
