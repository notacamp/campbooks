# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # What reaches the stack: the workspace filter strategy plus the starred /
      # blocked / allowed senders. Blacklist is the default strategy, so it reads
      # `default`; opting into whitelist reads `taught`. Sender facts are taught by
      # you; remove un-stars / un-blocks / neutralises the contact.
      class Filtering < Base
        def entries
          [ strategy_entry ] + contact_entries
        end

        def remove(entry)
          contact = entry.record
          return false unless contact.is_a?(Contact) && contact.workspace_id == workspace.id

          case entry.id
          when /\Ablocked:/ then contact.unblock!
          when /\Astarred:/ then contact.unstar!
          when /\Aallowed:/ then contact.update!(list_status: :neutral)
          else return false
          end
          true
        end

        private

        def strategy_entry
          whitelist = workspace.whitelist_mode?
          build(
            id: "filter:strategy",
            facet: :stack,
            sentence: sentence("scout_memory.sources.filtering.#{whitelist ? 'whitelist' : 'blacklist'}"),
            origin: whitelist ? :taught : :default,
            origin_detail: I18n.t("scout_memory.origins.#{whitelist ? 'taught' : 'default'}"),
            record: nil,
            form_path: routes.settings_inbox_section_path("filtering"),
            actions: %i[edit]
          )
        end

        def contact_entries
          result = []
          contacts.starred.order(:name, :email).limit(50).each do |contact|
            result << contact_entry(contact, "starred", contact.display_name.presence || contact.email)
          end
          contacts.blocked.order(:name, :email).limit(50).each do |contact|
            result << contact_entry(contact, "blocked", contact.email.presence || contact.display_name)
          end
          if workspace.whitelist_mode?
            contacts.allowed.where(starred_at: nil).order(:name, :email).limit(50).each do |contact|
              result << contact_entry(contact, "allowed", contact.display_name.presence || contact.email)
            end
          end
          result
        end

        def contact_entry(contact, kind, name)
          build(
            id: "#{kind}:#{contact.id}",
            facet: :stack,
            sentence: sentence("scout_memory.sources.filtering.#{kind}", name: name.to_s),
            origin: :taught,
            origin_detail: taught_detail(contact.starred_at || contact.updated_at),
            record: contact,
            form_path: routes.settings_inbox_section_path("filtering"),
            actions: %i[edit remove]
          )
        end

        def contacts
          workspace.contacts
        end
      end
    end
  end
end
