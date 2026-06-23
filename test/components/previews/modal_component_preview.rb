# frozen_string_literal: true

class ModalComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::Modal.new(open: true, size: :md)) do |modal|
      modal.with_header { "<h2 class=\"text-lg font-semibold text-gray-900\">Modal Title</h2>".html_safe }
      modal.with_body { "<p class=\"text-sm text-gray-600\">This is the modal body content. You can put any content here, including forms, tables, or text.</p>".html_safe }
      modal.with_footer do
        "<button type=\"button\" class=\"px-4 py-2 text-sm font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 transition-colors cursor-pointer\">Cancel</button>
         <button type=\"button\" class=\"px-4 py-2 text-sm font-medium rounded-lg bg-accent-600 text-white hover:bg-accent-700 transition-colors cursor-pointer\">Save</button>".html_safe
      end
    end
  end

  def small
    render(Campbooks::Modal.new(open: true, size: :sm)) do |modal|
      modal.with_header { "<h2 class=\"text-base font-semibold text-gray-900\">Quick Action</h2>".html_safe }
      modal.with_body { "<p class=\"text-sm text-gray-600\">A compact modal for simple confirmations or prompts.</p>".html_safe }
      modal.with_footer do
        "<button type=\"button\" class=\"px-3 py-1.5 text-sm font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 transition-colors cursor-pointer\">Cancel</button>
         <button type=\"button\" class=\"px-3 py-1.5 text-sm font-medium rounded-lg bg-accent-600 text-white hover:bg-accent-700 transition-colors cursor-pointer\">Confirm</button>".html_safe
      end
    end
  end

  def large
    render(Campbooks::Modal.new(open: true, size: :lg)) do |modal|
      modal.with_header { "<h2 class=\"text-lg font-semibold text-gray-900\">Large Modal</h2>".html_safe }
      modal.with_body do
        "<p class=\"text-sm text-gray-600\">This modal uses the large size option for more content.</p>
         <div class=\"mt-4 grid grid-cols-2 gap-4\">
           <div class=\"bg-gray-50 rounded-lg p-4\"><p class=\"text-sm font-medium text-gray-900\">Section One</p></div>
           <div class=\"bg-gray-50 rounded-lg p-4\"><p class=\"text-sm font-medium text-gray-900\">Section Two</p></div>
         </div>".html_safe
      end
    end
  end

  def closed
    render(Campbooks::Modal.new(open: false, size: :md)) do |modal|
      modal.with_header { "Hidden" }
      modal.with_body { "You should not see this." }
    end
  end
end
