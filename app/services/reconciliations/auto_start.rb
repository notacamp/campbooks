# frozen_string_literal: true

module Reconciliations
  # A bank statement Scout filed (from an email, or uploaded to Paper) becomes a
  # reconciliation on its own: nobody should have to upload a statement the app
  # already holds. Creates the Reconciliation and enqueues the parse (which then
  # matches). Says no, quietly, when the workspace can't reconcile, the document
  # isn't a statement with a file, it is already reconciled, or the same statement
  # (same file contents, or the same bank + period once extracted) already is.
  #
  #   Reconciliations::AutoStart.call(document)   # => Reconciliation, or nil with a reason
  class AutoStart
    Result = Struct.new(:reconciliation, :reason, keyword_init: true) do
      def started? = reconciliation.present?
    end

    def self.call(document, created_by: nil)
      new(document, created_by: created_by).call
    end

    def initialize(document, created_by: nil)
      @document   = document
      @workspace  = document.workspace
      @created_by = created_by
    end

    # The document row is locked while we decide, so the filing hook and a
    # "Reconcile them" click landing together still start one reconciliation.
    def call
      @document.with_lock do
        reason = blocker
        return Result.new(reason: reason) if reason

        reconciliation = @workspace.reconciliations.create!(
          created_by:         @created_by || default_creator,
          statement_document: @document,
          bank_name:          @document.try(:bank_name).presence || @document.metadata&.dig("bank_name"),
          currency:           @document.try(:currency).presence || "EUR"
        )
        Reconciliations::ParseJob.perform_later(reconciliation.id)
        Result.new(reconciliation: reconciliation)
      end
    end

    # Statements Scout holds that nobody has reconciled yet, newest first.
    def self.pending_for(workspace)
      workspace.documents
               .where(document_type: :bank_statement)
               .where.not(review_status: Document.review_statuses[:rejected])
               .where.not(id: Reconciliation.where(workspace_id: workspace.id).select(:statement_document_id))
               .where.associated(:original_file_attachment)
               .order(created_at: :desc)
    end

    private

    def blocker
      return :accounting_off      unless Features.accounting?
      return :not_entitled        unless @workspace.entitlements.allow?(:accounting) == :ok
      return :not_a_statement     unless @document.bank_statement?
      return :rejected            if @document.review_rejected?
      return :no_file             unless @document.original_file.attached?
      return :already_reconciled  if @document.reconciliations_as_statement.exists?
      return :duplicate_file      if duplicate_file?
      return :duplicate_period    if duplicate_period?
      return :no_user             if (@created_by || default_creator).nil?

      nil
    end

    # The same file already reconciled under another Document (a statement emailed twice).
    def duplicate_file?
      hash = @document.content_hash.presence
      return false unless hash

      twins = @workspace.documents.where(content_hash: hash).where.not(id: @document.id).select(:id)
      Reconciliation.where(workspace_id: @workspace.id, statement_document_id: twins).exists?
    end

    # Scout already extracted the statement's bank and period, and a ready
    # reconciliation covers exactly that period for that bank.
    def duplicate_period?
      from = @document.try(:period_start)
      to   = @document.try(:period_end)
      bank = @document.try(:bank_name).presence
      return false unless from.present? && to.present?

      scope = @workspace.reconciliations.where(status: :ready, period_start: from, period_end: to)
      scope = scope.where(bank_name: bank) if bank
      scope.exists?
    end

    def default_creator
      @default_creator ||= @workspace.users.order(:created_at).first
    end
  end
end
