class AddBuyerNifToDocuments < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :buyer_nif, :string
  end
end
