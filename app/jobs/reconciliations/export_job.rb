# frozen_string_literal: true

module Reconciliations
  # Builds and attaches a zip archive for a completed reconciliation.
  #
  # Lifecycle:
  #   export_none → export_generating → export_generated (success)
  #                                  → export_failed     (failure + re-raise)
  #
  # Broadcasts the export-button region before and after so the workbench UI
  # updates live without a page refresh. The broadcast is wrapped in the
  # reconciliation creator's locale so copy is localised.
  class ExportJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    def perform(reconciliation_id)
      @reconciliation = Reconciliation.find(reconciliation_id)
      Current.workspace = @reconciliation.workspace

      # The controller already set :export_generating synchronously before
      # enqueueing, so we just broadcast the current state here.
      broadcast_export_button!

      data = Reconciliations::ZipBuilder.new(@reconciliation).call

      bank   = @reconciliation.bank_name.presence || "statement"
      period = @reconciliation.period_label.presence || @reconciliation.created_at.strftime("%Y-%m")
      fname  = "reconciliation-#{bank}-#{period}.zip"
                 .downcase.gsub(/[^a-z0-9.\-]/, "-").squeeze("-")

      @reconciliation.export_zip.attach(
        io:           StringIO.new(data),
        filename:     fname,
        content_type: "application/zip"
      )

      @reconciliation.update!(export_status: :export_generated)
      broadcast_export_button!

      Notifier.reconciliation_export_ready(@reconciliation)

    rescue StandardError => e
      @reconciliation&.update_columns(
        export_status: Reconciliation.export_statuses[:export_failed],
        updated_at:    Time.current
      )
      broadcast_export_button!
      raise

    ensure
      Current.workspace = nil
    end

    private

    def broadcast_export_button!
      locale = @reconciliation.created_by&.locale.presence || I18n.default_locale
      html = I18n.with_locale(locale) do
        ApplicationController.render(
          partial: "reconciliations/export_button",
          locals:  { reconciliation: @reconciliation },
          layout:  false
        )
      end
      Turbo::StreamsChannel.broadcast_update_to(
        "reconciliation_#{@reconciliation.id}",
        target: "reconciliation_export_button",
        html:   html
      )
    rescue StandardError => e
      Rails.logger.warn("[Reconciliations::ExportJob] broadcast failed: #{e.class}: #{e.message}")
    end
  end
end
