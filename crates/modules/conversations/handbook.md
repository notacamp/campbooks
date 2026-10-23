## Mail and conversations

### Inboxes, conversations, messages

- An inbox is a connected mailbox. A member sees only the inboxes they belong to.
- An inbox can be shared. Each member of it has a role.
- Reader: can look, cannot change anything.
- Sender: can change conversations and send from the inbox.
- Manager: has at least the sender's rights.
- A conversation is a thread of messages. A message is inbound (received) or sent from the inbox.
- Mail in an inbox the member cannot see does not exist for them.

### Streams and places

Every conversation is in exactly one stream: Primary, Updates, Receipts or Newsletters. Scout places a new conversation from its first inbound message. If Scout is not sure, it stays in Primary. Later messages do not re-sort it. A move by the member sticks, in Campbooks or in their mail client.

- Receipts mail raises no need, unless a rule the member taught says it needs a reply or an action. The exception is a bill that may be owed.
- Newsletters mail raises no reply or Do need.
- Updates mail raises a reply or Do need only when it has a deadline.
- Primary is where reply and Do needs normally come from.

Places: Sent, Starred, Archive, Spam and Trash. Mail in Spam or Trash needs nothing. A need whose message moves there is cleared. Spam and Trash exclude each other. Mail in Spam never shows remote images.

Campbooks also mirrors a conversation's stream to the mail provider.

### How Scout reads a new message

Scout answers questions about each new inbound message:

- Which stream is it in?
- Does it need a reply?
- Does it ask for an action?
- Does it have a deadline?
- How urgent is it?
- Is it suspicious?
- Is money owed, or paid?
- Whose move is it next, and who is awaited?

Each answer has a confidence. An answer acts only when Scout is sure enough. Otherwise it is a suggestion. A suggestion changes nothing by itself. Its reasons are shown under "How Campbooks decided" in the app.

Facts from headers and addresses (list mail, automatic mail, no-reply senders) can only say no. They never decide what a message means. Nothing reads the words of a message with keyword rules. The model reads the words.

### Corrections

When a member moves a conversation to another stream, or answers a need with Not needed, Scout records a correction. Scout learns from corrections. A correction is not a rule.

### Taught rules

A member can teach Scout a rule. A rule has one to four conditions that must all hold and one to four actions.

- Conditions: sender address, sender domain, list, subject pattern or words, a header, a label, a stream.
- Actions: move to a stream, archive, add a label, mark read, needs a reply or not, asks an action or not, surface always or never, trash, spam, mark a bill paid, Highlight them (Worth a look), always show images from a sender.

Rules run first, before Scout's own reading, and always the same. The same message and the same rules always give the same result. A rule's answer overrides the model's.

- Suspicious mail is never hidden by a rule. A rule cannot archive it, mark it read, move it out of Primary or say its bill is paid. A rule that trashes it or reports it as spam may act on it.
- Surface never keeps a need from being raised. Surface always keeps other rules from archiving or moving the conversation.
- If the member changes back what a rule did to one conversation, that conversation becomes an exception. The rule never changes it again.
- Undoing a rule can put the mail it changed back as it was, except exceptions and mail changed since.
- A rule made from evidence (a correction or a suggestion) waits as a proposal for the member's yes, unless the member chose to let Campbooks act without asking.
- Only the member's own request says yes to a rule. You cannot.

### Scout noticed

For mail an inbox gets a lot of, Scout may offer a rule, such as "archive it as it arrives" or "keep it off Home". This is worked out from counts and facts, never from the text. It never covers a person, a sender the inbox writes to, or a sender that looked suspicious or asked for something. The member can say Not now (hidden for 14 days) or Never. Nothing is saved without the member's yes.

### Person notes

A member can tell Scout something about a person, in their own words. It is private to that member. Nobody else can read it, owners and admins included. Scout uses it for tone, form of address and language when writing a reply draft. It is not a fact to put in the reply.

### Reply drafts and Nudges

- Scout writes a reply or a Nudge only when the member asks. The result is a draft text. Nothing is saved or sent by that.
- Scout may propose up to three likely replies as short intents.
- The member edits and sends. A reader cannot send.
- Sending has an undo window of 10 seconds by default. Until it ends the member can take the send back.
- A draft made by Scout, an assistant or an automation can never be sent by them. Only the member's own Send sends.

### Waiting and Nudge

When the next move is someone else's, Campbooks keeps a Waiting need. Waiting is not in the needs list. It is shown apart. The member's inbox's own addresses are never awaited.

- Waiting is raised when the newest message raises no other need and Scout reads that the sender, or another person on the email, is expected to act.
- After a few working days (3 by default) the wait turns into a Nudge need. If the email gave a date, the Nudge comes the working day after it.
- The wait ends as "heard back" when that person writes to the same inbox.
- If the member writes to that person, the wait starts again from that message.
- Archiving does not end a wait. Trash, spam and the member resolving it do.

### Remote images

Remote images in a message are held back. They load only when the member chooses Show images, or when the member trusts the sender with "Always show images from". Trusting works only for a sender the receiving provider authenticates, and never for mail that looks suspicious. Tracking pixels are always removed. Mail in Spam never shows remote images.
