# Splits the single, overloaded Document#status enum into two orthogonal axes:
#
#   ai_status     — the AI processing lifecycle (pending/processing/completed/failed)
#   review_status — the human sign-off lifecycle (pending/approved/rejected)
#
# The old `review` meant BOTH "low-confidence success" AND "AI hard error", and the
# junk flag lived in a separate `dismissed_at` column. We disambiguate the two `review`
# meanings using ai_extraction_data: a hard error never reaches apply_result, so its
# extraction data is still the default `{}`; a low-confidence success has a populated
# hash. `dismissed_at` is absorbed into review_status: :rejected and dropped.
class SplitDocumentStatusIntoAiAndReview < ActiveRecord::Migration[8.1]
  def up
    add_column :documents, :ai_status, :integer, null: true
    add_column :documents, :review_status, :integer, null: true
    add_column :documents, :ai_error, :text

    # --- Backfill (old status -> two axes). Each row had a NOT NULL status in 0..5. ---

    # pending(0) -> ai pending, review pending
    execute <<~SQL.squish
      UPDATE documents SET ai_status = 0, review_status = 0 WHERE status = 0;
    SQL

    # processing(1) -> ai processing, review pending
    execute <<~SQL.squish
      UPDATE documents SET ai_status = 1, review_status = 0 WHERE status = 1;
    SQL

    # processed(2): high-confidence, auto-accepted, never human-reviewed. Honest
    # mapping is ai completed + review pending (surfaces them for the review the new
    # model promises). Flip review_status to 1 here if a clean go-forward queue is wanted.
    execute <<~SQL.squish
      UPDATE documents SET ai_status = 2, review_status = 0 WHERE status = 2;
    SQL

    # review(3) + dismissed: the junk flag -> completed + rejected (absorbs dismissed_at)
    execute <<~SQL.squish
      UPDATE documents SET ai_status = 2, review_status = 2
      WHERE status = 3 AND dismissed_at IS NOT NULL;
    SQL

    # review(3), not dismissed, with extraction data: a low-confidence success
    execute <<~SQL.squish
      UPDATE documents SET ai_status = 2, review_status = 0
      WHERE status = 3 AND dismissed_at IS NULL
        AND COALESCE(ai_extraction_data, '{}'::jsonb) <> '{}'::jsonb;
    SQL

    # review(3), not dismissed, no extraction data: an AI hard error parked as review
    execute <<~SQL.squish
      UPDATE documents
      SET ai_status = 3, review_status = 0, ai_error = 'AI analysis error — reprocess to retry'
      WHERE status = 3 AND dismissed_at IS NULL
        AND COALESCE(ai_extraction_data, '{}'::jsonb) = '{}'::jsonb;
    SQL

    # approved(4): human-signed-off -> completed + approved (reviewer stamps stay)
    execute <<~SQL.squish
      UPDATE documents SET ai_status = 2, review_status = 1 WHERE status = 4;
    SQL

    # failed(5): unhandled processing exception -> ai failed, review pending
    execute <<~SQL.squish
      UPDATE documents SET ai_status = 3, review_status = 0 WHERE status = 5;
    SQL

    change_column_null :documents, :ai_status, false, 0
    change_column_null :documents, :review_status, false, 0
    change_column_default :documents, :ai_status, from: nil, to: 0
    change_column_default :documents, :review_status, from: nil, to: 0

    # Old indexes reference columns we're about to drop — remove by name first.
    remove_index :documents, name: "index_documents_on_status"
    remove_index :documents, name: "index_documents_on_workspace_id_and_status_and_dismissed_at"

    remove_column :documents, :status
    remove_column :documents, :dismissed_at

    add_index :documents, [ :workspace_id, :review_status ]
    add_index :documents, [ :workspace_id, :ai_status ]
    add_index :documents, :review_status
    add_index :documents, :ai_status
    # The Skim review-queue query: review_status: pending, ordered by ai_confidence_score.
    add_index :documents, [ :workspace_id, :review_status, :ai_confidence_score ],
              name: "index_documents_on_workspace_review_confidence"
  end

  # Lossy rollback: the forward split collapses distinct old states (e.g. processed and
  # low-confidence review both -> completed/pending), so reverse mapping is best-effort.
  # ai_error text is discarded. Adequate for local rollback, not a data-faithful restore.
  def down
    add_column :documents, :status, :integer, null: true
    add_column :documents, :dismissed_at, :datetime

    # review_status drives the human axis on the way back
    execute "UPDATE documents SET status = 4 WHERE review_status = 1;" # approved
    execute "UPDATE documents SET status = 3, dismissed_at = updated_at WHERE review_status = 2;" # rejected -> review + dismissed

    # ai_status fills the rest where review_status is still pending(0)
    execute "UPDATE documents SET status = 0 WHERE review_status = 0 AND ai_status = 0;" # pending
    execute "UPDATE documents SET status = 1 WHERE review_status = 0 AND ai_status = 1;" # processing
    execute "UPDATE documents SET status = 3 WHERE review_status = 0 AND ai_status = 2;" # completed -> review
    execute "UPDATE documents SET status = 5 WHERE review_status = 0 AND ai_status = 3;" # failed
    execute "UPDATE documents SET status = 0 WHERE status IS NULL;" # safety net

    change_column_null :documents, :status, false, 0
    change_column_default :documents, :status, from: nil, to: 0

    remove_index :documents, name: "index_documents_on_workspace_review_confidence"
    remove_index :documents, column: :ai_status
    remove_index :documents, column: :review_status
    remove_index :documents, column: [ :workspace_id, :ai_status ]
    remove_index :documents, column: [ :workspace_id, :review_status ]

    remove_column :documents, :ai_status
    remove_column :documents, :review_status
    remove_column :documents, :ai_error

    add_index :documents, :status, name: "index_documents_on_status"
    add_index :documents, [ :workspace_id, :status, :dismissed_at ],
              name: "index_documents_on_workspace_id_and_status_and_dismissed_at"
  end
end
