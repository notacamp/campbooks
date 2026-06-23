class AddBulkSignalHeadersToEmailMessages < ActiveRecord::Migration[8.1]
  def change
    # Bulk / automated-mail signals captured from the message headers at ingest,
    # consumed by Emails::Categorizer to keep newsletters and machine mail out of
    # the "personal" bucket. Raw values (not just booleans) so List-Unsubscribe
    # can also drive a future one-click unsubscribe. All nullable: legacy mail and
    # providers that don't surface a given header simply leave it blank.
    add_column :email_messages, :header_list_unsubscribe, :text, comment: "RFC 2369 List-Unsubscribe value; presence => list/bulk mail"
    add_column :email_messages, :header_precedence, :string, comment: "RFC 2076 Precedence (bulk/list/junk => bulk mail)"
    add_column :email_messages, :header_auto_submitted, :string, comment: "RFC 3834 Auto-Submitted (anything but 'no' => machine-generated)"
  end
end
