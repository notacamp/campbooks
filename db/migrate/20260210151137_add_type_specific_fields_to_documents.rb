class AddTypeSpecificFieldsToDocuments < ActiveRecord::Migration[8.1]
  def change
    # Revenue invoice fields
    add_column :documents, :client_name, :string
    add_column :documents, :client_nif, :string

    # Bank statement fields
    add_column :documents, :bank_name, :string
    add_column :documents, :account_number, :string
    add_column :documents, :period_start, :date
    add_column :documents, :period_end, :date
    add_column :documents, :opening_balance_cents, :integer
    add_column :documents, :closing_balance_cents, :integer

    # Receipt fields
    add_column :documents, :payment_method, :string
    add_column :documents, :receipt_number, :string

    add_index :documents, :client_nif
  end
end
