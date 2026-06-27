module DocumentTemplates
  class Sender
    Result = Data.define(:ok, :pdf, :email_message, :error)
    def self.call(template:, variables:, to_address: nil, subject: nil, body: nil, user: nil, email_account_id: nil)
      new(template, variables, to_address, subject, body, user, email_account_id).call
    end
    def call
      filled = Filler.call(@template.html_content, @variables)
      pdf = PdfGenerator.call(filled)
      return Result.new(ok:true, pdf:pdf, email_message:nil, error:nil) if @to_address.blank?
      blob = ActiveStorage::Blob.create_and_upload!(io:StringIO.new(pdf), filename:"#{@template.name.parameterize}.pdf", content_type:"application/pdf")
      r = Emails::Sender.call(user:@user, to_address:@to_address, subject:@subject.presence||@template.name, body:@body.presence||"", email_account_id:@email_account_id, attachments:[blob])
      Result.new(ok:r.ok?, pdf:pdf, email_message:r.email_message, error:r.error_code)
    end
    private
    def initialize(t,v,ta,s,b,u,e)=(@template=t;@variables=v||{};@to_address=ta;@subject=s;@body=b;@user=u;@email_account_id=e)
  end
end
