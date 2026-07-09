# frozen_string_literal: true

class AddStatusIndexToBankTransactions < ActiveRecord::Migration[8.1]
  # Finding 13: index on (reconciliation_id, status) speeds up the status-filtered
  # COUNT queries used by the progress bar on the accounting index page. The
  # existing (reconciliation_id, position) index is unique and powers ordering;
  # this index serves the GROUP BY count query pattern.
  def change
    add_index :bank_transactions, [ :reconciliation_id, :status ],
              name: "index_bank_transactions_on_reconciliation_and_status"
  end
end
