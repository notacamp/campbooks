module DocumentTemplates
  class PdfGenerator
    M = { top:"15mm", bottom:"15mm", left:"15mm", right:"15mm" }.freeze
    def self.call(html)
      Grover.new(html, display_url:nil, print_background:true, format:"A4", margin:M, prefer_css_page_size:true, emulate_screen_media:false, wait_until:"networkidle0").to_pdf
    end
  end
end
