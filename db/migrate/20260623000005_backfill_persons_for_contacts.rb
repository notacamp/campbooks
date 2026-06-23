class BackfillPersonsForContacts < ActiveRecord::Migration[8.1]
  # Contacts created before eager Person creation have person_id = NULL and are
  # invisible on /contacts (the directory lists People, joined through contacts).
  # Give every person-less contact its own Person so it shows up; AI dedup merges
  # them afterwards. Self-contained models keep the migration valid on a fresh DB.
  class MigrationContact < ActiveRecord::Base
    self.table_name = "contacts"
  end

  class MigrationPerson < ActiveRecord::Base
    self.table_name = "people"
  end

  def up
    MigrationContact.where(person_id: nil).find_each do |contact|
      person = MigrationPerson.create!(workspace_id: contact.workspace_id)
      contact.update_columns(person_id: person.id)
    end
  end

  def down
    # Irreversible: backfilled Persons are indistinguishable from real ones.
  end
end
