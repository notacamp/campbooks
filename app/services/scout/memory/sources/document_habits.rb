# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # Learned document filing: "Documents from **Cloudhost** are usually
      # **Invoices**." Derived — like Documents::ClassificationMemory — from the
      # approved-documents corpus (sender_name → winning document_type), not a
      # separate table. The correction count feeds the origin label
      # ("Learned from N corrections").
      #
      # Confirm is an acknowledgement: the corpus already drives this, and the
      # human verdicts live in their own table, so there is nothing to write and
      # remove is not offered (it would only re-file real documents).
      class DocumentHabits < Base
        MIN_EXAMPLES = 3
        MIN_SHARE = 0.6

        def entries
          consensus.map { |row| entry_for(row) }
        end

        def confirm(_entry)
          true
        end

        private

        def consensus
          counts = workspace.documents
            .where(review_status: :approved)
            .where.not(document_type_id: nil)
            .group(Arel.sql("LOWER(documents.metadata->>'sender_name')"), :document_type_id)
            .count

          by_sender = Hash.new { |hash, key| hash[key] = {} }
          counts.each do |(sender, type_id), n|
            next if sender.blank?

            by_sender[sender][type_id] = n
          end

          by_sender.filter_map { |sender, type_counts| winning(sender, type_counts) }
        end

        def winning(sender, type_counts)
          total = type_counts.values.sum
          return nil if total < MIN_EXAMPLES

          type_id, count = type_counts.max_by { |_, value| value }
          return nil if count.to_f / total < MIN_SHARE

          type_name = DocumentType.where(id: type_id).pick(:name)
          return nil if type_name.blank?

          { sender: sender, type_name: type_name, count: count }
        end

        def entry_for(row)
          build(
            id: "dochabit:#{Scout::Memory::Entry.token(row[:sender])}",
            facet: :filing,
            sentence: sentence("scout_memory.sources.document_habits.sentence",
              sender: row[:sender].to_s.titleize, type: row[:type_name].to_s.humanize.pluralize),
            origin: :learned,
            origin_detail: I18n.t("scout_memory.origins.learned_corrections", count: row[:count]),
            record: nil,
            form_path: routes.settings_inbox_section_path("document_types"),
            actions: %i[confirm]
          )
        end
      end
    end
  end
end
