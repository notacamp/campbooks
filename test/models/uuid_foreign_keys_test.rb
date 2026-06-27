# frozen_string_literal: true

require "test_helper"

# Completeness guard for the bigint→uuid primary-key migration
# (db/migrate/*_migrate_primary_keys_to_uuid.rb).
#
# Every column that references a uuid-keyed table must itself be uuid. If a
# future migration adds a table or a `belongs_to` whose foreign key is left as
# bigint, joins/`where(id:)` silently break — this test fails first.
class UuidForeignKeysTest < ActiveSupport::TestCase
  # `belongs_to` associations that intentionally reference a column other than the
  # target's uuid primary key, so their FK column is NOT expected to be uuid.
  NON_PK_REFERENCES = [
    %w[Document email_message_id] # → email_messages.provider_message_id (a string)
  ].freeze

  test "every belongs_to onto a uuid table has a uuid foreign-key column" do
    Rails.application.eager_load!
    offenders = []

    ActiveRecord::Base.descendants.each do |model|
      next if model.abstract_class? || !model.table_exists?

      model.reflect_on_all_associations(:belongs_to).each do |assoc|
        next if assoc.polymorphic?
        next if assoc.options[:primary_key] # references a non-PK column
        next if NON_PK_REFERENCES.include?([ model.name, assoc.foreign_key.to_s ])

        target = begin
          assoc.klass
        rescue StandardError
          next
        end
        next unless column_type(target, target.primary_key) == :uuid

        fk = assoc.foreign_key.to_s
        next unless model.column_names.include?(fk)

        actual = column_type(model, fk)
        offenders << "#{model.table_name}.#{fk} is #{actual} (→ #{target.table_name}.id is uuid)" unless actual == :uuid
      end
    end

    assert_empty offenders,
                 "Foreign-key columns left non-uuid while their target is uuid:\n  #{offenders.join("\n  ")}"
  end

  # Polymorphic reference columns reach uuid tables too, so they must be uuid.
  test "polymorphic *_id columns that can reference uuid tables are uuid" do
    polymorphic = [
      %w[action_text_rich_texts record_id],
      %w[active_storage_attachments record_id],
      %w[agent_threads contextable_id],
      %w[audit_events target_id],
      %w[events actor_id],
      %w[events subject_id],
      %w[feed_items subject_id],
      %w[folder_memberships folderable_id],
      %w[notifications notifiable_id],
      %w[reminders source_id],
      %w[search_chunks searchable_id],
      %w[search_records searchable_id]
    ]
    conn = ActiveRecord::Base.connection
    offenders = polymorphic.filter_map do |table, col|
      type = conn.columns(table).find { |c| c.name == col }&.type
      "#{table}.#{col} is #{type}" unless type == :uuid
    end
    assert_empty offenders, "Polymorphic id columns must be uuid:\n  #{offenders.join("\n  ")}"
  end

  private

  def column_type(model, name)
    model.columns_hash[name.to_s]&.type
  end
end
