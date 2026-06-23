class ExportJob < ApplicationJob
  queue_as :default

  def perform(export_id)
    export = Export.find(export_id)
    export.update!(status: :generating)

    begin
      documents = export.workspace.documents
        .where(status: [ :processed, :approved ])
        .then { |scope| apply_filters(scope, export.filters) }
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

  def apply_filters(scope, filters)
    scope = scope.by_type(filters["type"]) if filters["type"].present?
    if filters["year"].present? && filters["month"].present?
      scope = scope.for_month(filters["year"].to_i, filters["month"].to_i)
    end
    scope
  end
end
