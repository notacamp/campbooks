# frozen_string_literal: true

# Paper (bold layout) settlement facts. A money document is "settled" (paid) either
# because a bank reconciliation confirmed a matching transaction (source
# `bank_match`, written by TransactionMatch#sync_document_settlement) or because a
# user marked it paid by hand (source `manual`, DocumentsController#settle). Kept as
# two plain columns rather than derived on the fly so Paper's status and the future
# Money surface can query and order by them.
class AddSettledToDocuments < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :settled_at, :datetime
    add_column :documents, :settled_source, :string
  end
end
