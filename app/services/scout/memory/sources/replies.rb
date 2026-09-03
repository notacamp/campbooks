# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # How Scout writes replies: your default signature, your stated writing style
      # (taught), the voice Scout learned from your sent mail (learned), and saved
      # reply templates when that feature is on.
      class Replies < Base
        def entries
          list = []
          list << signature_entry if default_signature
          list << writing_stated_entry if user.writing_style.present?
          list << writing_learned_entry if user.writing_style_learned.present?
          list.concat(template_entries) if Features.email_templates?
          list.compact
        end

        # Only the learned voice supports confirm/remove.
        def confirm(entry)
          entry.id == "writing:learned"
        end

        def remove(entry)
          return false unless entry.id == "writing:learned"

          user.update!(writing_style_learned: nil)
          true
        end

        private

        def default_signature
          return @default_signature if defined?(@default_signature)

          @default_signature = Signature.default_for(user)
        end

        def signature_entry
          build(
            id: "signature:#{default_signature.id}",
            facet: :replies,
            sentence: sentence("scout_memory.sources.replies.signature", name: default_signature.name),
            origin: :taught,
            origin_detail: taught_detail(default_signature.created_at),
            record: default_signature,
            form_path: routes.settings_inbox_section_path("signatures"),
            actions: %i[edit]
          )
        end

        def writing_stated_entry
          style = first_sentence(user.writing_style)
          return nil if style.blank?

          build(
            id: "writing:stated",
            facet: :replies,
            sentence: sentence("scout_memory.sources.replies.writing_stated", style: style),
            origin: :taught,
            origin_detail: taught_detail(user.writing_style_updated_at),
            record: nil,
            form_path: routes.settings_account_path,
            actions: %i[edit]
          )
        end

        def writing_learned_entry
          summary = first_sentence(user.writing_style_learned)
          return nil if summary.blank?

          build(
            id: "writing:learned",
            facet: :replies,
            sentence: sentence("scout_memory.sources.replies.writing_learned", summary: summary),
            origin: :learned,
            origin_detail: I18n.t("scout_memory.origins.learned_style"),
            record: nil,
            form_path: routes.settings_account_path,
            actions: %i[confirm remove]
          )
        end

        def template_entries
          EmailTemplate.where(workspace_id: workspace.id).usable.order(:name).map do |template|
            build(
              id: "template:#{template.id}",
              facet: :replies,
              sentence: sentence("scout_memory.sources.replies.template", name: template.name),
              origin: :taught,
              origin_detail: taught_detail(template.created_at),
              record: template,
              form_path: routes.settings_email_templates_path,
              actions: %i[edit]
            )
          end
        end
      end
    end
  end
end
