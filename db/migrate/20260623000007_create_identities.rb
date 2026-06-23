class CreateIdentities < ActiveRecord::Migration[8.1]
  def change
    create_table :identities do |t|
      t.references :user, null: false, foreign_key: true
      # Plain string (not an enum) holding "google" / "microsoft" / "zoho" — the
      # same value Auth::OauthSignIn passes, so lookups never need a cast.
      t.string :provider, null: false
      # Stable provider account id (Google GAIA id, Microsoft AAD object id, Zoho
      # accountId). Identities are keyed on this, NOT email — emails change.
      t.string :uid, null: false
      # The address at link time. Display only; never used for matching.
      t.string :email

      t.timestamps
    end

    add_index :identities, [ :provider, :uid ], unique: true
  end
end
