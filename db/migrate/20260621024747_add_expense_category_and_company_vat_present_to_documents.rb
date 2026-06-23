class AddExpenseCategoryAndCompanyVatPresentToDocuments < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :expense_category, :integer
    add_column :documents, :company_vat_present, :boolean
  end
end
