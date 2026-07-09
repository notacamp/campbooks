class AddAiMatchDebugToBankTransactions < ActiveRecord::Migration[8.1]
  def change
    # Per-transaction audit of the AI disambiguation run: candidates sent,
    # raw matches claimed by the model, and each grounding decision
    # (kept / capped / discarded / twin_collapsed). Nullable — most
    # transactions resolve heuristically and never consult the AI. The raw
    # HTTP exchange lives in external_service_calls (SystemHealth middleware);
    # this column is the domain-level view for debugging bad suggestions.
    add_column :bank_transactions, :ai_match_debug, :jsonb
  end
end
