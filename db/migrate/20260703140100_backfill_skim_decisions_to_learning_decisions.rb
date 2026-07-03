class BackfillSkimDecisionsToLearningDecisions < ActiveRecord::Migration[8.1]
  # Copy existing skim_decisions into the generic learning_decisions table so the
  # new Learning:: substrate keeps every user's already-learned Skim habits. This
  # runs automatically on db:prepare / boot — self-hosters need no manual rake or
  # backfill step. COPY ONLY: skim_decisions is left fully intact and is dropped in
  # a later release once row-count parity has been confirmed. Replayable from zero
  # (an empty source table simply copies zero rows).
  def up
    return unless table_exists?(:skim_decisions)

    execute(<<~SQL)
      INSERT INTO learning_decisions
        (id, domain, workspace_id, user_id, label, contact_id, sender_domain,
         category, subject_type, subject_id, signals, created_at, updated_at)
      SELECT
        gen_random_uuid(),
        'email_skim',
        workspace_id,
        user_id,
        action,
        contact_id,
        sender_domain,
        category,
        CASE WHEN email_message_id IS NOT NULL THEN 'EmailMessage' END,
        email_message_id,
        '{}'::jsonb,
        created_at,
        NOW()
      FROM skim_decisions
    SQL
  end

  def down
    execute("DELETE FROM learning_decisions WHERE domain = 'email_skim'")
  end
end
