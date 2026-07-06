# frozen_string_literal: true

# Tracks the user's review decision for each provider label discovered during
# label sync. Decisions:
#   pending  — seen but not yet reviewed
#   mapped   — user linked this label to an existing workspace tag
#   kept     — user kept it as its own workspace tag (or it was pre-existing)
#   ignored  — user decided to leave this label unmapped (no workspace tag)
#
# Once a decision is recorded the review banner stays silent for that label,
# even across re-syncs. Additive — no existing data is touched.
class CreateLabelImportDecisions < ActiveRecord::Migration[8.1]
  def change
    create_table :label_import_decisions, id: :uuid do |t|
      t.references :email_account, null: false, foreign_key: true, type: :uuid
      t.string     :provider_label_id,   null: false
      t.string     :provider_label_name, null: false
      t.integer    :decision,            null: false, default: 0  # see enum in model
      t.references :tag,                 null: true,  foreign_key: true, type: :uuid
      t.references :reviewed_by,         null: true,  foreign_key: { to_table: :users }, type: :uuid
      t.datetime   :reviewed_at

      t.timestamps
    end

    # One decision row per provider label per account.
    add_index :label_import_decisions, [ :email_account_id, :provider_label_id ],
              unique: true, name: "idx_label_import_decisions_account_label"
  end
end
