class AddProviderLabelsToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    # The provider's raw label ids for the message, captured at ingest (Gmail
    # labelIds today; empty for providers without an equivalent). Feeds
    # EmailMessage#provider_category_hint so triage can use Gmail's own
    # category verdicts without depending on the label→tag sync.
    add_column :email_messages, :provider_labels, :jsonb, default: [], null: false
  end
end
