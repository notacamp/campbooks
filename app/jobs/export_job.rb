class ExportJob < ApplicationJob
  queue_as :default

  def perform(export_id)
    export = Export.find(export_id)
    export.update!(status: :generating)

    begin
      # Base scope: documents whose AI analysis finished and that haven't been
      # explicitly rejected by a human. Replaces the now-removed `status` column.
      base = export.workspace.documents
        .where(ai_status: :completed, review_status: [ :pending, :approved ])

      # Apply stored filters, normalising legacy hashes from old export records.
      filters = Documents::Filters.from_params(normalize_legacy_filters(export.filters))
      # Exports are workspace-wide — no per-user folder permission check needed
      # (exports never carry folder-name params; folder_id still works by id).
      documents = filters.apply(base, workspace: export.workspace, user: nil)
        .includes(:classification)

      export.update!(documents_count: documents.count)

      if documents.any?
        zip_data = Exports::ZipGenerator.new(documents).call

        export.zip_file.attach(
          io: StringIO.new(zip_data),
          filename: "export_#{export.created_at.strftime('%Y%m%d_%H%M%S')}.zip",
          content_type: "application/zip"
        )
      end

      export.update!(status: :generated)
      Notifier.export_ready(export)
    rescue => e
      export.update!(status: :failed)
      Notifier.export_failed(export)
      Rails.logger.error("[ExportJob] Error: #{e.message}")
      raise
    end
  end

  private

  # Normalise filter hashes stored by older versions of the export action so
  # Documents::Filters.from_params can consume them unchanged.
  #
  # Legacy shape (pre-refactor):
  #   { "year" => "2026", "month" => "6", "type" => "<id>", "category" => "..." }
  #
  # Normalized shape:
  #   { "month" => "2026-06", "type" => ["<id>"], "category" => "..." }
  def normalize_legacy_filters(stored)
    return {} if stored.blank?

    h = stored.stringify_keys

    # year + month → "YYYY-MM" month param
    if h["year"].present? && h["month"].present?
      month_str = format("%04d-%02d", h["year"].to_i, h["month"].to_i)
      h = h.except("year", "month").merge("month" => month_str)
    end

    # Scalar type → single-element array (Filters expects array)
    if h["type"].present? && !h["type"].is_a?(Array)
      h["type"] = [ h["type"] ]
    end

    h
  end
end
