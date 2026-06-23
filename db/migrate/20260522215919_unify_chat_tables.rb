class UnifyChatTables < ActiveRecord::Migration[8.1]
  def up
    # 1. Add columns to agent_threads
    add_column :agent_threads, :purpose, :integer, default: 0, null: false
    add_column :agent_threads, :contextable_type, :string
    add_column :agent_threads, :contextable_id, :integer
    add_index :agent_threads, [ :contextable_type, :contextable_id ]
    add_index :agent_threads, :purpose

    # 2. Add columns to agent_messages
    add_column :agent_messages, :draft, :boolean, default: false, null: false
    add_column :agent_messages, :outdated, :boolean, default: false, null: false
    add_column :agent_messages, :reply_status, :integer

    # 3. Add new FK column to email_messages
    add_column :email_messages, :ai_analysis_message_id, :bigint

    # 4. Migrate data
    say_with_time "migrating email_threads -> agent_threads" do
      migrate_threads
    end

    say_with_time "migrating email_comments -> agent_messages" do
      migrate_comments
    end

    say_with_time "backfilling ai_analysis_message_id" do
      backfill_analysis_message_ids
    end

    # 5. Remove old FK and add new one
    remove_column :email_messages, :ai_analysis_comment_id
    add_index :email_messages, :ai_analysis_message_id
    add_foreign_key :email_messages, :agent_messages, column: :ai_analysis_message_id

    # 6. Drop email_comments
    drop_table :email_comments
  end

  def down
    # No-op: keep migrated data intact on rollback
  end

  private

  def migrate_threads
    # Find the owner user for each email_account (prefer owner, then first user with access)
    account_owners = {}
    execute(<<~SQL).each { |r| account_owners[r["id"]] = r["user_id"] }
      SELECT DISTINCT ON (e.id) e.id, eau.user_id
      FROM email_accounts e
      INNER JOIN email_account_users eau ON eau.email_account_id = e.id
      WHERE eau.owner = true OR eau.can_read = true
      ORDER BY e.id, eau.owner DESC
    SQL

    org_ids = execute("SELECT id, organization_id FROM email_accounts").each_with_object({}) { |r, h|
      h[r["id"]] = r["organization_id"]
    }

    results = execute("SELECT id, subject, email_account_id FROM email_threads")

    results.each do |et|
      owner_id = account_owners[et["email_account_id"]]
      org_id = org_ids[et["email_account_id"]]
      next if owner_id.nil?

      existing = execute(<<~SQL).first
        SELECT id FROM agent_threads
        WHERE purpose = 1 AND contextable_type = 'EmailThread' AND contextable_id = #{et["id"]}
        LIMIT 1
      SQL

      if existing.nil?
        execute(<<~SQL)
          INSERT INTO agent_threads (title, purpose, contextable_type, contextable_id, user_id, organization_id, created_at, updated_at)
          VALUES (#{quote(et["subject"])}, 1, 'EmailThread', #{et["id"]}, #{owner_id}, #{org_id || 'NULL'}, NOW(), NOW())
        SQL
      end
    end
  end

  def migrate_comments
    # Build mapping of old email_thread_id -> { new_agent_thread_id, thread_user_id }
    thread_map = {}
    execute(<<~SQL).each { |r| thread_map[r["id"]] = { new_id: r["new_id"], user_id: r["user_id"] } }
      SELECT et.id AS id, at2.id AS new_id, at2.user_id AS user_id
      FROM email_threads et
      INNER JOIN agent_threads at2 ON at2.contextable_type = 'EmailThread' AND at2.contextable_id = et.id AND at2.purpose = 1
    SQL

    results = execute(<<~SQL)
      SELECT id, email_thread_id, user_id, author_type, content, draft, outdated, reply_status,
             ai_auto_actions, ai_suggested_actions, created_at, updated_at
      FROM email_comments
      ORDER BY created_at ASC
    SQL

    # Store mapping from old email_comment ID to new agent_message ID
    @comment_message_map = {}

    results.each do |ec|
      at_info = thread_map[ec["email_thread_id"]]
      next if at_info.nil?
      agent_thread_id = at_info[:new_id]
      # AI-authored comments have no user_id; fall back to the thread owner
      msg_user_id = ec["user_id"] || at_info[:user_id]

      result = execute(<<~SQL)
        INSERT INTO agent_messages (agent_thread_id, user_id, author_type, content, draft, outdated, reply_status,
               ai_auto_actions, ai_suggested_actions, created_at, updated_at)
        VALUES (#{agent_thread_id}, #{msg_user_id}, #{ec["author_type"]}, #{quote(ec["content"])},
                #{ec["draft"]}, #{ec["outdated"]}, #{ec["reply_status"]},
                #{quote(ec["ai_auto_actions"])}, #{quote(ec["ai_suggested_actions"])},
                #{quote(ec["created_at"])}, #{quote(ec["updated_at"])})
        RETURNING id
      SQL
      new_id = result.first["id"]
      @comment_message_map[ec["id"]] = new_id
    end
  end

  def backfill_analysis_message_ids
    results = execute("SELECT id, ai_analysis_comment_id FROM email_messages WHERE ai_analysis_comment_id IS NOT NULL")
    results.each do |em|
      new_message_id = @comment_message_map[em["ai_analysis_comment_id"]]
      next if new_message_id.nil?
      execute("UPDATE email_messages SET ai_analysis_message_id = #{new_message_id} WHERE id = #{em['id']}")
    end
  end
end
