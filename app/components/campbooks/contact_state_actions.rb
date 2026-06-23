# frozen_string_literal: true

module Campbooks
  # Compact star + block/unblock controls for a Contact, used inside the app-wide
  # hover card (Campbooks::ContactPopover) so a sender can be starred or blocked
  # from anywhere their avatar appears. Each button posts to
  # contacts#set_state; the response (set_state.turbo_stream) re-renders this same
  # wrapper in place (id: contact_popover_actions_<id>), so the buttons flip
  # without a reload. Blocking confirms first (it archives the sender's mail).
  #
  # The richer profile-header equivalent lives in contacts/_state_actions.html.erb;
  # both speak to the same endpoint and i18n (contacts.state.*).
  class ContactStateActions < Campbooks::Base
    def initialize(contact:)
      @contact = contact
    end

    def view_template
      div(id: "contact_popover_actions_#{@contact.id}", class: "flex items-center gap-2") do
        star_button
        @contact.blocked? ? unblock_button : block_button
      end
    end

    private

    def star_button
      starred = @contact.starred?
      action_form(state: starred ? "unstar" : "star") do
        button(
          type: "submit",
          class: button_classes(
            starred ? "border-amber-300 bg-amber-50 text-amber-700 dark:bg-amber-500/10 dark:border-amber-500/30 dark:text-amber-300" : nil
          ),
          aria_pressed: starred.to_s
        ) do
          raw(safe(helpers.contact_star_icon(filled: starred).to_s))
          plain(t(starred ? "contacts.state.starred" : "contacts.state.star"))
        end
      end
    end

    def block_button
      action_form(state: "block", confirm: t("contacts.state.block_confirm")) do
        button(
          type: "submit",
          class: button_classes("hover:bg-red-50 hover:text-red-600 hover:border-red-200 dark:hover:bg-red-500/10")
        ) do
          raw(safe(helpers.contact_block_icon.to_s))
          plain(t("contacts.state.block"))
        end
      end
    end

    def unblock_button
      action_form(state: "unblock") do
        button(type: "submit", class: button_classes) do
          raw(safe(helpers.contact_unblock_icon.to_s))
          plain(t("contacts.state.unblock"))
        end
      end
    end

    def action_form(state:, confirm: nil, &block)
      data = confirm ? { turbo_confirm: confirm } : {}
      form(
        action: helpers.set_state_contact_path(@contact, state: state),
        method: "post",
        class: "contents",
        data: data
      ) do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token, autocomplete: "off")
        yield
      end
    end

    def button_classes(extra = nil)
      class_names(
        "flex-1 inline-flex items-center justify-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-xs font-medium transition-colors cursor-pointer",
        extra || "border-border text-muted-foreground hover:bg-muted"
      )
    end
  end
end
