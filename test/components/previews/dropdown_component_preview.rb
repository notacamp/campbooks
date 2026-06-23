# frozen_string_literal: true

class DropdownComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::Dropdown.new) do |dropdown|
      dropdown.with_trigger do
        tag.span(class: "flex items-center gap-2") do
          render(Campbooks::Avatar.new(name: "Jane Doe", size: :sm))
          tag.span("Jane Doe", class: "text-sm text-gray-700")
        end
      end
      dropdown.with_menu do
        tag.div(class: "px-3 py-2 border-b border-gray-100") do
          tag.p("Jane Doe", class: "text-sm font-medium text-gray-900") +
          tag.p("jane@example.com", class: "text-xs text-gray-500")
        end +
        tag.div(class: "py-1") do
          tag.a("Account Settings", href: "#", class: "block px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50")
        end +
        tag.div(class: "border-t border-gray-100 py-1") do
          tag.button("Sign out", type: "button", class: "block w-full text-left px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50")
        end
      end
    end
  end

  def left_aligned
    render(Campbooks::Dropdown.new(placement: :left)) do |dropdown|
      dropdown.with_trigger do
        tag.span("Options", class: "text-sm text-gray-700")
      end
      dropdown.with_menu do
        tag.a("Edit", href: "#", class: "block px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50") +
        tag.a("Delete", href: "#", class: "block px-3 py-1.5 text-sm text-red-600 hover:bg-gray-50")
      end
    end
  end

  def simple_menu
    render(Campbooks::Dropdown.new) do |dropdown|
      dropdown.with_trigger do
        tag.span("Actions", class: "text-sm font-medium text-gray-700")
      end
      dropdown.with_menu do
        tag.a("Copy", href: "#", class: "block px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50") +
        tag.a("Move", href: "#", class: "block px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50") +
        tag.div(class: "border-t border-gray-100") +
        tag.a("Delete", href: "#", class: "block px-3 py-1.5 text-sm text-red-600 hover:bg-gray-50")
      end
    end
  end
end
