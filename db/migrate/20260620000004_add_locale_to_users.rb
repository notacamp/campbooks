class AddLocaleToUsers < ActiveRecord::Migration[8.1]
  # Per-user UI language preference. Nil means "fall back to the request's
  # Accept-Language, then I18n.default_locale" (resolved in ApplicationController).
  def change
    add_column :users, :locale, :string
  end
end
